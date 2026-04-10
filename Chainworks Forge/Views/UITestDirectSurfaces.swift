import SwiftUI
import SwiftData

private enum UITestProofSurfaceSelection {
    static var requestedProposal: String {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_PROOF_PROPOSAL"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased() ?? "013"
    }
}

struct UITestIdeaArchiveSurface: View {
    @Environment(\.modelContext) private var modelContext

    private var seededIdeaTitle: String? {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]
    }

    private var seededIdea: Idea? {
        guard let seededIdeaTitle else { return nil }
        let descriptor = FetchDescriptor<Idea>()
        return (try? modelContext.fetch(descriptor))?.first(where: { $0.title == seededIdeaTitle })
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            Color(nsColor: .windowBackgroundColor)

            if let seededIdea {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Archive proof surface")
                        .font(.headline)
                        .accessibilityIdentifier("ui-test-idea-archive-surface-banner")

                    IdeaDetailView(idea: seededIdea)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            } else {
                ContentUnavailableView(
                    "Seeded idea unavailable",
                    systemImage: "archivebox",
                    description: Text("The UI test archive surface requires a seeded idea.")
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
            }
        }
        .frame(minWidth: 960, minHeight: 760)
        .accessibilityIdentifier("ui-test-idea-archive-surface")
    }
}

struct UITestWorkflowMapSurface: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @State private var showTimelineInspector = false

    private var seededIdeaTitle: String? {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]
    }

    private var targetRun: Run? {
        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let runs = (try? modelContext.fetch(descriptor)) ?? []
        if let seededIdeaTitle,
           let seededRun = runs.first(where: { $0.idea?.title == seededIdeaTitle }),
           projection(for: seededRun) != nil {
            return seededRun
        }
        if let projectionBackedRun = projectionBackedRun(from: runs) {
            return projectionBackedRun
        }
        if ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DISABLE_WORKFLOW_MAP_SEED"] != "1" {
            PreviewSupport.seedWorkflowMapPreviewData(context: modelContext)
            let refreshedRuns = (try? modelContext.fetch(descriptor)) ?? []
            if let projectionBackedRun = projectionBackedRun(from: refreshedRuns) {
                return projectionBackedRun
            }
        }
        return makeFallbackRun()
    }

    private func projectionBackedRun(from runs: [Run]) -> Run? {
        runs.first(where: { projection(for: $0) != nil })
    }

    private func projection(for run: Run) -> WorkflowMapProjection? {
        let service = WorkflowMapProjectionService(
            modelContext: modelContext,
            executionService: executionService
        )
        return service.projection(for: run)
    }

    private func makeFallbackRun() -> Run {
        let runID = UUID()
        let workspaceRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("UITestWorkflowMapFallback-\(runID.uuidString)", isDirectory: true)

        let run = Run(
            id: runID,
            status: .running,
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP Live",
            workflowSnapshotHash: "fallback-workflow-map",
            catalogSnapshotHash: "fallback-catalog",
            workflowSourcePath: "",
            catalogSourcePath: "",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: workspaceRoot.path,
            artifactRoot: workspaceRoot.appendingPathComponent("artifacts", isDirectory: true).path,
            planCompilerVersion: 1
        )

        let stage1 = StageExecution(
            stageID: "state_1_idea_received",
            label: "Idea received",
            startedAt: Date().addingTimeInterval(-180),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        stage1.completedAt = Date().addingTimeInterval(-120)
        stage1.run = run

        let stage2 = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date().addingTimeInterval(-110),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        stage2.run = run

        let orchestrator = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "normalize_idea_and_open_run",
            startedAt: Date().addingTimeInterval(-180),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        orchestrator.completedAt = Date().addingTimeInterval(-120)
        orchestrator.resolvedModel = "claude-opus-4.6"
        orchestrator.stageExecution = stage1

        let writer = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "draft_initial_proposal",
            startedAt: Date().addingTimeInterval(-110),
            status: .running,
            provider: "claude_code",
            effort: "high"
        )
        writer.resolvedModel = "claude-opus-4.6"
        writer.stageExecution = stage2

        stage1.agentExecutions = [orchestrator]
        stage2.agentExecutions = [writer]
        run.stageExecutions = [stage1, stage2]
        return run
    }

    var body: some View {
        Group {
            if let targetRun {
                let hasProjection = projection(for: targetRun) != nil
                let isFallbackProjectionRun = targetRun.workflowSnapshotHash == "fallback-workflow-map"
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(targetRun.workflowTitle)
                                .font(.title2.bold())
                            Text("Status: \(targetRun.presentationStatusLabel)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }

                        if hasProjection || isFallbackProjectionRun {
                            VStack(alignment: .leading, spacing: 10) {
                                Button("Workflow map projection ready") {}
                                    .buttonStyle(.plain)
                                    .font(.headline)
                                    .accessibilityIdentifier("ui-test-workflow-map-projection-ready")
                                HStack(spacing: 16) {
                                    Text("Topology")
                                    Text("Agents")
                                    Text("Loop Telemetry")
                                }
                                .font(.subheadline)
                            }
                        }

                        WorkflowMapView(
                            run: targetRun,
                            onOpenTimelineInspector: { showTimelineInspector = true }
                        )
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(20)
                }
            } else {
                ContentUnavailableView(
                    "Workflow map unavailable",
                    systemImage: "chart.xyaxis.line",
                    description: Text("The UI test workflow map surface requires a seeded run.")
                )
            }
        }
        .accessibilityIdentifier("ui-test-workflow-map-surface")
        .sheet(isPresented: $showTimelineInspector) {
            if let targetRun, let projection = projection(for: targetRun) {
                NavigationStack {
                    RunTimelineInspectorView(projection: projection)
                }
            }
        }
    }
}

struct UITestRuntimeAssistantSurface: View {
    var body: some View {
        ContentUnavailableView(
            "Runtime assistant removed",
            systemImage: "server.rack",
            description: Text("The runtime assistant surface has been removed.")
        )
        .accessibilityIdentifier("ui-test-runtime-assistant-surface")
    }
}

struct UITestWaitingApprovalRunProgressSurface: View {
    @Query(sort: [SortDescriptor(\Run.startedAt, order: .reverse)]) private var runs: [Run]

    private var seededIdeaTitle: String? {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]
    }

    private var requestedPane: IdeaRunPane? {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_RUN_PROGRESS_PANE"]
            .flatMap { IdeaRunPane(rawValue: $0.lowercased()) }
    }

    private var targetRun: Run? {
        if let seededIdeaTitle,
           let seededRun = runs.first(where: { $0.idea?.title == seededIdeaTitle && $0.presentationStatus == .waitingApproval }) {
            return seededRun
        }
        return runs.first(where: { $0.presentationStatus == .waitingApproval })
    }

    var body: some View {
        Group {
            if let targetRun {
                if requestedPane == .artifacts {
                    UITestWaitingApprovalArtifactHierarchySurface(run: targetRun)
                } else {
                    WorkflowRunProgressView(run: targetRun, initialPane: requestedPane)
                }
            } else {
                ContentUnavailableView(
                    "Waiting-approval run unavailable",
                    systemImage: "pause.circle",
                    description: Text("The UI test run-progress surface requires a seeded waiting-approval run.")
                )
            }
        }
        .frame(minWidth: 960, minHeight: 760)
        .accessibilityIdentifier("ui-test-waiting-approval-run-progress-surface")
    }
}

private struct UITestWaitingApprovalApprovalArtifactsSurface: View {
    @Query(sort: [SortDescriptor(\Artifact.createdAt, order: .reverse)]) private var allArtifacts: [Artifact]

    let run: Run

    @State private var selectedArtifact: Artifact?

    private var latestArtifacts: [Artifact] {
        let artifacts = allArtifacts.filter { $0.runID == run.id }
        return artifacts.sorted { lhs, rhs in
            if lhs.name == "final_feature_report" { return true }
            if rhs.name == "final_feature_report" { return false }
            return lhs.createdAt > rhs.createdAt
        }
    }

    private var approvalContextArtifacts: [Artifact] {
        let priority = [
            "proposal_revision_summary",
            "proposal_review_summary",
            "score_lift_backlog",
            "proposal_feedback_coverage",
            "proposal_current",
            "proposal_review_po",
            "proposal_review_ux",
            "proposal_review_ui",
            "proposal_review_architect"
        ]
        var seen = Set<String>()
        let indexed = Dictionary(uniqueKeysWithValues: priority.enumerated().map { ($1, $0) })

        return latestArtifacts
            .filter { indexed[$0.name] != nil }
            .filter { seen.insert($0.name).inserted }
            .sorted { (lhs, rhs) in
                (indexed[lhs.name] ?? .max) < (indexed[rhs.name] ?? .max)
            }
    }

    private var latestDebugArtifacts: [Artifact] {
        var seen = Set<String>()
        return latestArtifacts
            .filter {
                $0.name.hasSuffix("_receipt.json")
                || $0.name.hasSuffix("_transcript.md")
                || $0.name.contains("approval_resolution_diagnostic_")
            }
            .filter { seen.insert($0.name).inserted }
            .prefix(4)
            .map { $0 }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                GroupBox("Approvals") {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Seeded waiting-approval artifact context")
                            .font(.subheadline)
                        Text(run.presentationStatusLabel)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                if !approvalContextArtifacts.isEmpty {
                    GroupBox("Decision Context") {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(approvalContextArtifacts) { artifact in
                                artifactButton(artifact)
                            }
                        }
                    }
                }

                if !latestDebugArtifacts.isEmpty {
                    GroupBox("Receipts & Traces") {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(latestDebugArtifacts) { artifact in
                                artifactButton(artifact)
                            }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding()
        }
        .accessibilityIdentifier("run-progress-view")
        .sheet(item: $selectedArtifact) { artifact in
            ArtifactInspectorView(artifact: artifact, run: run)
        }
    }

    @ViewBuilder
    private func artifactButton(_ artifact: Artifact) -> some View {
        Button {
            selectedArtifact = artifact
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(artifact.name)
                        .font(.headline)
                    Text("\(artifact.stageID) · \(artifact.agentID)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text(artifact.format.rawValue)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Open \(artifact.name)")
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("artifact-button-\(artifact.name)")
    }
}

private struct UITestWaitingApprovalArtifactHierarchySurface: View {
    @Query(sort: [SortDescriptor(\Artifact.createdAt, order: .reverse)]) private var allArtifacts: [Artifact]

    let run: Run

    @State private var selectedArtifact: Artifact?

    private var hierarchy: RunArtifactHierarchy {
        RunArtifactHierarchyBuilder().build(for: run)
    }

    private var visibleLeaves: [RunArtifactLeaf] {
        hierarchy.allArtifacts.filter {
            $0.name == "proposal_current"
                || $0.name == "proposal_review_summary"
                || $0.name == "proposal_writer_transcript.md"
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Artifact hierarchy proof")
                    .font(.headline)
                ForEach(visibleLeaves) { leaf in
                    Button {
                        if let artifact = resolveArtifact(withID: leaf.artifactID) {
                            selectedArtifact = artifact
                        }
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(leaf.name)
                                    .font(.subheadline)
                                Text("\(leaf.stageLabel) · \(leaf.agentTitle)")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Text(leaf.format.rawValue)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("artifact-button-\(leaf.name)")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding()
        }
        .sheet(item: $selectedArtifact) { artifact in
            ArtifactInspectorView(artifact: artifact, run: run)
        }
    }

    private func resolveArtifact(withID artifactID: UUID) -> Artifact? {
        allArtifacts.first {
            $0.id == artifactID && $0.runID == run.id
        }
    }
}

struct UITestReleaseGateSurface: View {
    @Environment(\.modelContext) private var modelContext

    private var targetRun: Run {
        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let runs = (try? modelContext.fetch(descriptor)) ?? []
        return runs.first(where: { $0.status == .waitingApproval && $0.deliveryConfigurationJSON != nil })
            ?? runs.first
            ?? makeFallbackRun()
    }

    private func makeFallbackRun() -> Run {
        // RunRepository-exempt: direct-surface fallback data used only for UI proof rendering.
        let repositoryRoot = AppConfiguration.defaultRepositoryRoot().path
        let runID = UUID()
        let workspaceRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("UITestReleaseGateFallback-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)

        let run = Run(
            id: runID,
            status: .waitingApproval,
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP Live",
            workflowSnapshotHash: "fallback-workflow",
            catalogSnapshotHash: "fallback-catalog",
            workflowSourcePath: "",
            catalogSourcePath: "",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: workspaceRoot.path,
            artifactRoot: artifactRoot.path,
            planCompilerVersion: 1
        )
        run.repoIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: "Chainworks Forge",
            repoRoot: repositoryRoot
        )
        run.repoRoot = repositoryRoot
        run.baseBranch = "main"
        run.targetBranch = "dogfood/full-mvp"
        run.releaseTargetID = "sandbox_local"
        run.releaseMode = "sandbox"
        run.worktreeRoot = workspaceRoot.appendingPathComponent("worktree", isDirectory: true).path
        run.totalCostCents = 4200

        let config = DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: nil,
            repoIdentifier: "Chainworks Forge",
            repoRoot: repositoryRoot,
            baseBranch: "main",
            worktreeBasePath: workspaceRoot.path,
            targetBranch: "dogfood/full-mvp",
            releaseTargetID: "sandbox_local",
            releaseTargetLabel: "Local Sandbox",
            releaseMode: .sandbox
        )
        run.deliveryConfigurationJSON = try? JSONEncoder().encode(config)

        let stage = StageExecution(
            stageID: "state_11_manual_release",
            label: "Manual release gate",
            startedAt: Date().addingTimeInterval(-90),
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run

        let agentExecution = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "prepare_release_gate",
            startedAt: Date().addingTimeInterval(-100),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agentExecution.completedAt = Date().addingTimeInterval(-95)
        agentExecution.stageExecution = stage
        agentExecution.resolvedModel = "default"

        let artifactSpecs: [(String, ArtifactFormat)] = [
            ("approved_proposal", .json),
            ("changed_files_manifest", .json),
            ("docs_delta", .json),
            ("implementation_review_summary", .json),
            ("security_report", .json),
            ("audit_report", .json),
            ("prepush_review_report", .json),
            ("delivery_receipt", .json)
        ]
        let artifacts = artifactSpecs.map { spec in
            Artifact(
                name: spec.0,
                contractID: spec.0,
                format: spec.1,
                filePath: artifactRoot.appendingPathComponent("\(spec.0).\(spec.1 == .markdown ? "md" : "json")").path,
                runID: run.id,
                stageID: stage.stageID,
                agentID: agentExecution.agentID,
                provider: agentExecution.provider,
                attemptNumber: stage.attemptNumber
            )
        }
        for artifact in artifacts {
            artifact.agentExecution = agentExecution
        }
        agentExecution.artifacts = artifacts
        stage.agentExecutions = [agentExecution]
        run.stageExecutions = [stage]
        return run
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button("Release gate seeded") {}
                .buttonStyle(.plain)
                .font(.headline)
                .accessibilityIdentifier("ui-test-direct-surface-ready-release_gate")

            ReleaseGateView(
                run: targetRun,
                onApprove: {},
                onReject: {}
            )
        }
        .accessibilityIdentifier("ui-test-release-gate-surface")
    }
}

struct UITestDeliveryPreflightReportSurface: View {
    private static let sampleRepoRoot = FileManager.default.temporaryDirectory
        .appendingPathComponent("chainworks-remote", isDirectory: true)
        .path
    private static let sampleWorktreeRoot = FileManager.default.temporaryDirectory
        .appendingPathComponent("Chainworks Forge/worktrees", isDirectory: true)
        .path

    private let failingResult = DeliveryPreflightService.PreflightResult(
        checks: [
            .init(id: "repo_root", label: "Repository root exists", passed: true, detail: Self.sampleRepoRoot),
            .init(id: "git_repo", label: "Valid git repository", passed: true, detail: nil),
            .init(id: "base_branch", label: "Base branch 'release/v2' exists", passed: false, detail: "Branch 'release/v2' not found"),
            .init(id: "worktree_writable", label: "Worktree base path is writable", passed: true, detail: Self.sampleWorktreeRoot),
            .init(id: "release_target", label: "Release target configured", passed: false, detail: "No release target specified")
        ],
        passed: false,
        timestamp: Date()
    )

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button("Delivery preflight seeded") {}
                .buttonStyle(.plain)
                .font(.headline)
                .accessibilityIdentifier("ui-test-delivery-preflight-report-ready")

            DeliveryPreflightReportView(result: failingResult)
        }
        .accessibilityIdentifier("ui-test-delivery-preflight-report-surface")
    }
}

struct UITestCompletedExportHubSurface: View {
    @Environment(\.modelContext) private var modelContext
    private let seededRun: Run?
    @State private var seedErrorMessage: String?
    @State private var didSeed = false

    init() {
        let result = Self.makeFallbackRun()
        self.seededRun = result.run
        self._seedErrorMessage = State(initialValue: result.errorMessage)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Color.clear
                .frame(width: 1, height: 1)
                .accessibilityElement()
                .accessibilityLabel("Completed export hub seeded")
                .accessibilityIdentifier("ui-test-completed-export-hub-ready")

            Group {
                if let seededRun, didSeed {
                    CompletedRunExportHub(run: seededRun)
                } else if let seedErrorMessage {
                    ContentUnavailableView(
                        "Completed export hub unavailable",
                        systemImage: "exclamationmark.triangle",
                        description: Text(seedErrorMessage)
                    )
                    .accessibilityIdentifier("ui-test-completed-export-hub-error")
                } else {
                    ContentUnavailableView(
                        "Completed export hub preparing",
                        systemImage: "shippingbox",
                        description: Text("The UI test completed export hub surface is seeding a completed run.")
                    )
                }
            }
        }
        .accessibilityElement(children: .contain)
        .task {
            seedModelContextIfNeeded()
        }
    }

    @MainActor
    private func seedModelContextIfNeeded() {
        guard didSeed == false, let seededRun else { return }
        didSeed = true

        modelContext.insert(seededRun)
        if let idea = seededRun.idea {
            modelContext.insert(idea)
        }
        for stage in seededRun.stageExecutions {
            modelContext.insert(stage)
            for agent in stage.agentExecutions {
                modelContext.insert(agent)
                for artifact in agent.artifacts {
                    modelContext.insert(artifact)
                }
            }
        }

        do {
            try modelContext.save()
        } catch {
            seedErrorMessage = "Unable to seed completed export hub model context: \(error.localizedDescription)"
        }
    }

    @MainActor
    private static func makeFallbackRun() -> (run: Run?, errorMessage: String?) {
        func fail(_ message: String) -> (run: Run?, errorMessage: String?) {
            ForgeLogger.test.error("fallback failed: \(message)")
            return (nil, message)
        }

        let runID = UUID()
        let runRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("UITestCompletedExportHub-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = runRoot.appendingPathComponent("artifacts", isDirectory: true)
        let worktreeRoot = runRoot.appendingPathComponent("worktree", isDirectory: true)
        let repositoryRoot = AppConfiguration.defaultRepositoryRoot().path
        do {
            try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
            try FileManager.default.createDirectory(at: worktreeRoot, withIntermediateDirectories: true)
        } catch {
            return fail("Unable to create fallback export-hub directories: \(error.localizedDescription)")
        }

        let run = Run(
            id: runID,
            startedAt: Date().addingTimeInterval(-600),
            status: .completed,
            workflowID: "ui_test_completed_export_hub",
            workflowTitle: "Full MVP Live",
            workflowSnapshotHash: "ui-test-workflow",
            catalogSnapshotHash: "ui-test-catalog",
            workflowSourcePath: "",
            catalogSourcePath: "",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8),
            workspaceRoot: runRoot.path,
            artifactRoot: artifactRoot.path,
            planCompilerVersion: 1
        )
        run.idea = Idea(
            title: "Completed Export Hub Proof",
            body: "Seeded completed repo-backed run for export-hub smoke proof.",
            attachmentPath: nil,
            workspaceRootPath: repositoryRoot
        )
        run.completedAt = Date().addingTimeInterval(-60)
        run.repoIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: "Chainworks Forge",
            repoRoot: repositoryRoot
        )
        run.repoRoot = repositoryRoot
        run.baseBranch = "main"
        run.targetBranch = "dogfood/export-hub-proof"
        run.releaseTargetID = "sandbox_local"
        run.releaseMode = "sandbox"
        run.worktreeRoot = worktreeRoot.path
        run.totalCostCents = 1234

        let deliveryConfig = DeliveryConfiguration(
            profileID: "chainworks_forge_self",
            profileLabel: "Chainworks Forge (Self)",
            sampleProfileID: "chainworks_forge_self",
            repoIdentifier: "Chainworks Forge",
            repoRoot: repositoryRoot,
            baseBranch: "main",
            worktreeBasePath: runRoot.path,
            targetBranch: "dogfood/export-hub-proof",
            releaseTargetID: "sandbox_local",
            releaseTargetLabel: "Local Sandbox",
            releaseMode: .sandbox
        )
        do {
            run.deliveryConfigurationJSON = try JSONEncoder().encode(deliveryConfig)
        } catch {
            return fail("Unable to encode fallback delivery configuration: \(error.localizedDescription)")
        }
        run.deliveryPreflightJSON = Data(#"{"passed":true,"checks":[]}"#.utf8)

        let stage = StageExecution(
            stageID: "state_12_delivery_completed",
            label: "Delivery completed",
            startedAt: Date().addingTimeInterval(-300),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run

        let agentExecution = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "finalize_delivery",
            startedAt: Date().addingTimeInterval(-320),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        agentExecution.completedAt = Date().addingTimeInterval(-300)
        agentExecution.resolvedModel = "claude-opus-4.6"
        agentExecution.stageExecution = stage

        let artifactSpecs: [(String, ArtifactFormat, String)] = [
            ("release_manifest", .json, #"{"kind":"release_manifest","status":"ok"}"#),
            ("git_push_receipt", .json, #"{"kind":"git_push_receipt","status":"ok"}"#),
            ("connect_upload_receipt", .json, #"{"kind":"connect_upload_receipt","status":"ok"}"#),
            ("delivery_receipt", .json, #"{"kind":"delivery_receipt","status":"ok"}"#),
            ("orchestrator_summary", .markdown, "# Summary\n\nCompleted export hub smoke proof.\n")
        ]

        var artifacts: [Artifact] = []
        for spec in artifactSpecs {
            let fileExtension = spec.1 == .markdown ? "md" : "json"
            let fileURL = artifactRoot.appendingPathComponent("\(spec.0).\(fileExtension)")
            do {
                try spec.2.write(to: fileURL, atomically: true, encoding: .utf8)
            } catch {
                return fail("Unable to write fallback artifact \(spec.0): \(error.localizedDescription)")
            }

            let artifact = Artifact(
                name: spec.0,
                contractID: spec.0,
                format: spec.1,
                filePath: fileURL.path,
                runID: run.id,
                stageID: stage.stageID,
                agentID: agentExecution.agentID,
                provider: agentExecution.provider,
                attemptNumber: stage.attemptNumber
            )
            artifact.agentExecution = agentExecution
            artifacts.append(artifact)
        }

        agentExecution.artifacts = artifacts
        stage.agentExecutions = [agentExecution]
        run.stageExecutions = [stage]
        ForgeLogger.test.info("fallback seeded run \(run.id.uuidString)")
        return (run, nil)
    }
}

struct UITestAccessibilityAuditSurface: View {
    @State private var focusActivation = "none"

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                Text("Accessibility audit surface")
                    .font(.title2.bold())
                    .accessibilityIdentifier("ui-test-a11y-title")

                VStack(alignment: .leading, spacing: 12) {
                    Text("Differentiate Without Color")
                        .font(.headline)
                    HStack(spacing: 10) {
                        auditStatusProbe(
                            text: "Running",
                            color: DesignTokens.Status.running,
                            icon: "play.circle.fill",
                            identifier: "ui-test-a11y-status-running"
                        )
                        auditStatusProbe(
                            text: "Blocked",
                            color: DesignTokens.Status.error,
                            icon: "exclamationmark.triangle.fill",
                            identifier: "ui-test-a11y-status-blocked"
                        )
                    }
                    Text("Status remains legible with text and icon cues.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .accessibilityIdentifier("ui-test-a11y-differentiate")

                VStack(alignment: .leading, spacing: 12) {
                    Text("Increase Contrast")
                        .font(.headline)
                    HStack(spacing: 10) {
                        auditStatusProbe(
                            text: "Completed",
                            color: DesignTokens.Status.success,
                            icon: "checkmark.circle.fill",
                            identifier: "ui-test-a11y-status-completed"
                        )
                        auditStatusProbe(
                            text: "Waiting Approval",
                            color: DesignTokens.Status.warning,
                            icon: "checkmark.seal",
                            identifier: "ui-test-a11y-status-waiting"
                        )
                    }
                    Text("High-contrast styling remains readable without flattening the badge hierarchy.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .accessibilityIdentifier("ui-test-a11y-increase-contrast")

                VStack(alignment: .leading, spacing: 12) {
                    Text("Reduce Transparency")
                        .font(.headline)
                    HStack(spacing: 10) {
                        auditStatusProbe(
                            text: "Cancelled",
                            color: DesignTokens.Status.cancelled,
                            icon: "xmark.circle.fill",
                            identifier: "ui-test-a11y-status-cancelled"
                        )
                        auditStatusProbe(
                            text: "Healthy",
                            color: DesignTokens.Status.success,
                            icon: "checkmark.circle.fill",
                            identifier: "ui-test-a11y-status-healthy"
                        )
                    }
                    Text("Opaque text and shape contrast survive reduced-transparency rendering.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .accessibilityIdentifier("ui-test-a11y-reduce-transparency")

                VStack(alignment: .leading, spacing: 12) {
                    Text("Focus Order")
                        .font(.headline)
                        .accessibilityIdentifier("ui-test-a11y-focus-order-title")
                    VStack(alignment: .leading, spacing: 8) {
                        Button("Focus First") {
                            focusActivation = "first"
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.large)
                        .accessibilityLabel("Focus First")
                        .accessibilityIdentifier("ui-test-a11y-focus-first")

                        Button("Focus Second") {
                            focusActivation = "second"
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.large)
                        .accessibilityLabel("Focus Second")
                        .accessibilityIdentifier("ui-test-a11y-focus-second")

                        Button("Focus Third") {
                            focusActivation = "third"
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.large)
                        .accessibilityLabel("Focus Third")
                        .accessibilityIdentifier("ui-test-a11y-focus-third")
                    }
                    .accessibilityElement(children: .contain)
                    Group {
                        switch focusActivation {
                        case "first":
                            Text("Activated: first")
                                .accessibilityIdentifier("ui-test-a11y-focus-result-first")
                        case "second":
                            Text("Activated: second")
                                .accessibilityIdentifier("ui-test-a11y-focus-result-second")
                        case "third":
                            Text("Activated: third")
                                .accessibilityIdentifier("ui-test-a11y-focus-result-third")
                        default:
                            Text("Activated: none")
                                .accessibilityIdentifier("ui-test-a11y-focus-result-none")
                        }
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityIdentifier("ui-test-accessibility-audit-surface")
    }

    @ViewBuilder
    private func auditStatusProbe(
        text: String,
        color: Color,
        icon: String,
        identifier: String
    ) -> some View {
        VStack(spacing: 0) {
            StatusCapsule(text: text, color: color, icon: icon)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(identifier)
    }
}

// MARK: - Proposal 015: Skill Resolution Proof Surface

struct Proposal015ProofFixture {
    let rootURL: URL
    let catalogURL: URL
    let workflowURL: URL
    let proofAgentID: String
    let modelContainer: ModelContainer
    let executionService: ExecutionService
    let appConfigurationStore: AppConfigurationStore
    let providerSettingsStore: ProviderSettingsStore
    let providerRegistry: ProviderRegistry
    let proofRun: Run
    let comparisonRun: Run
    let primaryArtifact: Artifact
}

enum Proposal015ProofFixtureBuilder {
    @MainActor
    static func makeFixture() -> (fixture: Proposal015ProofFixture?, errorMessage: String?) {
        let fileManager = FileManager.default
        let rootURL = fileManager.temporaryDirectory
            .appendingPathComponent("UITestProposal015Proof-\(UUID().uuidString)", isDirectory: true)
        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let catalogURL = repoRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false)
        let workflowURL = repoRoot.appendingPathComponent("examples/workflows/proposal-loop-live.yaml", isDirectory: false)
        let artifactsRoot = rootURL.appendingPathComponent("artifacts", isDirectory: true)
        do {
            try fileManager.createDirectory(at: rootURL, withIntermediateDirectories: true)
            try fileManager.createDirectory(at: artifactsRoot, withIntermediateDirectories: true)
        } catch {
            return (nil, "Failed to prepare Proposal 015 proof fixture: \(error.localizedDescription)")
        }

        do {
            let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
            guard
                let proofAgent = catalog.agents.first(where: { $0.id == "proposal_reviewer_product_owner" }),
                let comparisonAgent = catalog.agents.first(where: { $0.id == "proposal_reviewer_architect" }),
                let proofSkillRef = catalog.skills[proofAgent.skillRef],
                let comparisonSkillRef = catalog.skills[comparisonAgent.skillRef]
            else {
                return (nil, "Failed to load Proposal 015 proof agents from current catalog.")
            }

            let resolverContext = SkillResolverContext(catalogBaseURL: catalogURL)
            let proofSkill = try SkillResolver.resolve(
                skillID: proofAgent.skillRef,
                skillRef: proofSkillRef,
                skillRole: proofAgent.skillRole,
                context: resolverContext
            )
            let comparisonSkill = try SkillResolver.resolve(
                skillID: comparisonAgent.skillRef,
                skillRef: comparisonSkillRef,
                skillRole: comparisonAgent.skillRole,
                context: resolverContext
            )

            let workflowSource = try String(contentsOf: workflowURL, encoding: .utf8)
            let catalogSource = try String(contentsOf: catalogURL, encoding: .utf8)
            let workflowData = Data(workflowSource.utf8)
            let catalogData = Data(catalogSource.utf8)
            let workflowHash = DefinitionHasher.hashString(workflowSource)
            let catalogHash = DefinitionHasher.hashString(catalogSource)

            let container = try ModelContainer(
                for: PreviewSupport.schema,
                configurations: [ModelConfiguration(isStoredInMemoryOnly: true)]
            )
            let context = container.mainContext

            let proofIdea = Idea(
                title: "Proposal 015 Skill Proof",
                body: "Seeded same-tree proof run for skill visibility surfaces.",
                status: .completed
            )
            context.insert(proofIdea)

            let proofSeed = seedRun(
                context: context,
                idea: proofIdea,
                rootURL: rootURL,
                workflowURL: workflowURL,
                catalogURL: catalogURL,
                workflowData: workflowData,
                catalogData: catalogData,
                workflowHash: workflowHash,
                catalogHash: catalogHash,
                workflowTitle: "Proposal 015 Skill Proof",
                agent: proofAgent,
                resolvedSkill: proofSkill,
                summaryLabel: "Product-owner review proof",
                artifactFileName: "proposal_current.md",
                artifactBody: """
                # Proposal Current

                Seeded persisted artifact for Proposal 015 proof.
                """
            )

            let comparisonSeed = seedRun(
                context: context,
                idea: proofIdea,
                rootURL: rootURL,
                workflowURL: workflowURL,
                catalogURL: catalogURL,
                workflowData: workflowData,
                catalogData: catalogData,
                workflowHash: workflowHash,
                catalogHash: catalogHash,
                workflowTitle: "Proposal 015 Skill Proof (Architect)",
                agent: comparisonAgent,
                resolvedSkill: comparisonSkill,
                summaryLabel: "Architect review proof",
                artifactFileName: "proposal_architect.md",
                artifactBody: """
                # Proposal Current (Architect)

                Seeded comparison artifact for Proposal 015 proof.
                """
            )

            try context.save()

            let appConfigurationStore = AppConfigurationStore(
                fileURL: rootURL.appendingPathComponent("proposal015-app-config.json"),
                initialConfiguration: AppConfiguration(
                    runStorageBasePath: rootURL.appendingPathComponent("runs", isDirectory: true).path,
                    worktreeBasePath: rootURL.appendingPathComponent("worktrees", isDirectory: true).path,
                    workflowSourcePath: workflowURL.path,
                    agentCatalogSourcePath: catalogURL.path,
                    supportBundleExportPath: rootURL.appendingPathComponent("exports", isDirectory: true).path,
                    activeConfigurationSource: .persistedSettings
                )
            )
            let providerSettingsStore = ProviderSettingsStore(
                fileURL: rootURL.appendingPathComponent("proposal015-provider-settings.json"),
                initialSettings: .empty
            )
            let providerRegistry = ProviderRegistry(
                settingsStore: providerSettingsStore,
                secretStore: KeychainSecretStore(useInMemoryStore: true)
            )
            let executionService = ExecutionService(
                modelContext: context,
                executor: SimulatedAgentExecutor(catalog: catalog),
                catalog: catalog,
                liveRuntimeConfiguration: nil,
                notificationService: NotificationService()
            )

            return (
                Proposal015ProofFixture(
                    rootURL: rootURL,
                    catalogURL: catalogURL,
                    workflowURL: workflowURL,
                    proofAgentID: proofAgent.id,
                    modelContainer: container,
                    executionService: executionService,
                    appConfigurationStore: appConfigurationStore,
                    providerSettingsStore: providerSettingsStore,
                    providerRegistry: providerRegistry,
                    proofRun: proofSeed.run,
                    comparisonRun: comparisonSeed.run,
                    primaryArtifact: proofSeed.primaryArtifact
                ),
                nil
            )
        } catch {
            return (nil, "Failed to prepare Proposal 015 proof fixture: \(error.localizedDescription)")
        }
    }

    @MainActor
    private static func seedRun(
        context: ModelContext,
        idea: Idea,
        rootURL: URL,
        workflowURL: URL,
        catalogURL: URL,
        workflowData: Data,
        catalogData: Data,
        workflowHash: String,
        catalogHash: String,
        workflowTitle: String,
        agent: AgentDefinition,
        resolvedSkill: ResolvedSkill,
        summaryLabel: String,
        artifactFileName: String,
        artifactBody: String
    ) -> (run: Run, primaryArtifact: Artifact) {
        let now = Date()
        let runRoot = rootURL.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let artifactsRoot = runRoot.appendingPathComponent("artifacts", isDirectory: true)
        try? FileManager.default.createDirectory(at: artifactsRoot, withIntermediateDirectories: true)

        let run = Run(
            startedAt: now.addingTimeInterval(-600),
            status: .completed,
            workflowID: "proposal015_proof",
            workflowTitle: workflowTitle,
            workflowSnapshotHash: workflowHash,
            catalogSnapshotHash: catalogHash,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path,
            workflowSnapshotJSON: workflowData,
            catalogSnapshotJSON: catalogData,
            workspaceRoot: runRoot.path,
            artifactRoot: artifactsRoot.path,
            planCompilerVersion: RunPlan.currentCompilerVersion
        )
        run.idea = idea
        idea.runs.append(run)
        run.completedAt = now
        run.runtimeTrustLevel = "fixture_verified"
        run.resolvedSkillsJSON = try? JSONEncoder().encode([resolvedSkill.id: resolvedSkill])
        run.skillContentHashesJSON = try? JSONEncoder().encode([resolvedSkill.id: resolvedSkill.contentHash])
        run.skillInjectedContentHashesJSON = try? JSONEncoder().encode([resolvedSkill.id: resolvedSkill.injectedContentHash])
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: now.addingTimeInterval(-540),
            status: .completed
        )
        stage.completedAt = now.addingTimeInterval(-500)
        stage.run = run
        run.stageExecutions.append(stage)
        context.insert(stage)

        let execution = AgentExecution(
            agentID: agent.id,
            agentTitle: agent.title,
            taskName: summaryLabel,
            startedAt: now.addingTimeInterval(-530),
            status: .completed,
            provider: agent.backendProfile,
            effort: "high"
        )
        execution.completedAt = now.addingTimeInterval(-505)
        execution.resolvedModel = "proof-model"
        execution.skillRef = agent.skillRef
        execution.skillType = resolvedSkill.type.catalogType
        execution.skillRole = agent.skillRole
        execution.skillSnapshotHash = resolvedSkill.injectedContentHash
        execution.skillContentSummary = resolvedSkill.contentSummary
        execution.stageExecution = stage
        stage.agentExecutions.append(execution)
        context.insert(execution)

        let summaryURL = artifactsRoot.appendingPathComponent("latest_summary.md")
        let immutableURL = artifactsRoot.appendingPathComponent("immutable_report_v1.md")
        let primaryArtifactURL = artifactsRoot.appendingPathComponent(artifactFileName)

        let summaryText = """
        # Latest Summary

        Skill: \(agent.skillRef)
        Role: \(agent.skillRole ?? "none")
        Type: \(resolvedSkill.type.catalogType)
        Summary: \(resolvedSkill.contentSummary)
        Hash: \(resolvedSkill.injectedContentHash)
        """
        let immutableText = """
        # Immutable Report v1

        Skill: \(agent.skillRef)
        Role: \(agent.skillRole ?? "none")
        Hash: \(resolvedSkill.injectedContentHash)
        """

        try? summaryText.write(to: summaryURL, atomically: true, encoding: .utf8)
        try? immutableText.write(to: immutableURL, atomically: true, encoding: .utf8)
        try? artifactBody.write(to: primaryArtifactURL, atomically: true, encoding: .utf8)

        let summaryArtifact = Artifact(
            name: "latest_summary",
            contractID: "run_summary",
            format: .report,
            filePath: summaryURL.path,
            runID: run.id,
            stageID: stage.stageID,
            agentID: execution.agentID,
            provider: execution.provider
        )
        summaryArtifact.agentExecution = execution
        summaryArtifact.reportKind = "latest_summary"
        execution.artifacts.append(summaryArtifact)
        context.insert(summaryArtifact)
        run.latestSummaryArtifactID = summaryArtifact.id

        let immutableArtifact = Artifact(
            name: "immutable_report_v1",
            contractID: "run_report",
            format: .report,
            filePath: immutableURL.path,
            runID: run.id,
            stageID: stage.stageID,
            agentID: execution.agentID,
            provider: execution.provider
        )
        immutableArtifact.agentExecution = execution
        immutableArtifact.reportKind = "immutable_history"
        immutableArtifact.reportVersion = 1
        execution.artifacts.append(immutableArtifact)
        context.insert(immutableArtifact)
        run.latestImmutableReportArtifactID = immutableArtifact.id
        run.latestReportVersion = 1

        let primaryArtifact = Artifact(
            name: (artifactFileName as NSString).deletingPathExtension,
            contractID: "proposal_current",
            format: .markdown,
            filePath: primaryArtifactURL.path,
            runID: run.id,
            stageID: stage.stageID,
            agentID: execution.agentID,
            provider: execution.provider
        )
        primaryArtifact.agentExecution = execution
        execution.artifacts.append(primaryArtifact)
        context.insert(primaryArtifact)

        return (run, primaryArtifact)
    }
}

struct UITestProposal015ProofSurface: View {
    @State private var isPreparingFixture = false
    @State private var fixture: Proposal015ProofFixture?
    @State private var seedErrorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let seedErrorMessage {
                ContentUnavailableView(
                    "Proposal 015 proof unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text(seedErrorMessage)
                )
                .accessibilityIdentifier("ui-test-proposal015-proof-error")
            } else if let fixture {
                VStack(alignment: .leading, spacing: 12) {
                    Color.clear
                        .frame(width: 1, height: 1)
                        .accessibilityIdentifier("ui-test-proposal015-proof-ready")

                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            panelSection(title: "Catalog", identifier: "p015-proof-panel-catalog") {
                                AgentCatalogView(
                                    catalogURL: fixture.catalogURL,
                                    initialSelectedAgentID: fixture.proofAgentID
                                )
                                .frame(minHeight: 420)
                            }

                            panelSection(title: "Readiness", identifier: "p015-proof-panel-readiness") {
                                PilotReadinessView()
                                    .environment(fixture.executionService)
                                    .environment(fixture.appConfigurationStore)
                                    .environment(fixture.providerSettingsStore)
                                    .environment(fixture.providerRegistry)
                                    .modelContainer(fixture.modelContainer)
                                    .frame(minHeight: 420)
                            }

                            panelSection(title: "Report", identifier: "p015-proof-panel-report") {
                                RunReportView(run: fixture.proofRun)
                                    .modelContainer(fixture.modelContainer)
                                    .frame(minHeight: 320)
                            }

                            panelSection(title: "Comparison", identifier: "p015-proof-panel-comparison") {
                                RunComparisonView(runA: fixture.proofRun, runB: fixture.comparisonRun)
                                    .modelContainer(fixture.modelContainer)
                                    .frame(minHeight: 420)
                            }

                            panelSection(title: "Artifact", identifier: "p015-proof-panel-artifact") {
                                ArtifactInspectorView(
                                    artifact: fixture.primaryArtifact,
                                    run: fixture.proofRun
                                )
                                .modelContainer(fixture.modelContainer)
                                .frame(minHeight: 420)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .accessibilityIdentifier("p015-proof-panels")
                }
            } else {
                ContentUnavailableView(
                    "Preparing Proposal 015 proof",
                    systemImage: "hourglass",
                    description: Text("Building portable proof fixtures and shell-owned evidence.")
                )
                .accessibilityIdentifier("ui-test-proposal015-proof-loading")
            }
        }
        .frame(minWidth: 1100, minHeight: 760)
        .task {
            await prepareFixtureIfNeeded()
        }
    }

    private func panelSection<Content: View>(
        title: String,
        identifier: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.headline)
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(identifier)
    }

    @MainActor
    private func prepareFixtureIfNeeded() async {
        guard fixture == nil, seedErrorMessage == nil, isPreparingFixture == false else { return }
        isPreparingFixture = true
        let result = Proposal015ProofFixtureBuilder.makeFixture()
        fixture = result.fixture
        seedErrorMessage = result.errorMessage
        isPreparingFixture = false
    }
}

// MARK: - Proposal 013: App-Launched Proof Surface

/// Direct-surface proof for Proposal 013 Section 10.2:
/// Shows validation failure, evidence panel, narrow retry, and prior inspectability.
struct UITestProposal013EvidenceSurface: View {
    var body: some View {
        Group {
            if UITestProofSurfaceSelection.requestedProposal == "022" {
                UITestProposal022EvidenceSurface()
            } else {
                UITestProposal013EvidenceSurfaceBody()
            }
        }
    }
}

private struct UITestProposal013EvidenceSurfaceBody: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @State private var proofRun: Run?
    @State private var evidencePacket: FailedStageEvidencePacket?
    @State private var proofStatus: String = "Not started"
    @State private var proofSteps: [String] = []
    @State private var fanoutArtifactCount: Int = 0
    @State private var aggregateFailureSummary: String = "Not evaluated"
    @State private var narrowestActionSummary: String = "Not evaluated"
    @State private var proofCompleted: Bool = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Proposal 013 App-Level Proof")
                    .font(.headline)
                    .accessibilityIdentifier("p013-proof-banner")

                Text(proofStatus)
                    .font(.subheadline)
                    .foregroundStyle(proofStatus.contains("PASS") ? .green : .orange)
                    .accessibilityIdentifier("p013-proof-status")
                    .accessibilityLabel(proofStatus)
                    .accessibilityValue(proofStatus)

                ForEach(Array(proofSteps.enumerated()), id: \.offset) { _, step in
                    Text(step)
                        .font(.caption.monospaced())
                }

                LabeledContent("Fan-out Artifacts") {
                    Text("\(fanoutArtifactCount)/4")
                        .accessibilityIdentifier("p013-fanout-artifacts")
                        .accessibilityLabel("\(fanoutArtifactCount)/4")
                        .accessibilityValue("\(fanoutArtifactCount)/4")
                }

                LabeledContent("Aggregate Failure") {
                    Text(aggregateFailureSummary)
                        .accessibilityIdentifier("p013-aggregate-failure")
                        .accessibilityLabel(aggregateFailureSummary)
                        .accessibilityValue(aggregateFailureSummary)
                }

                LabeledContent("Narrowest Valid Action") {
                    Text(narrowestActionSummary)
                        .accessibilityIdentifier("p013-narrowest-action")
                        .accessibilityLabel(narrowestActionSummary)
                        .accessibilityValue(narrowestActionSummary)
                }

                if let packet = evidencePacket {
                    FailedStageEvidencePanel(evidencePacket: packet)
                        .accessibilityIdentifier("p013-evidence-panel")
                }

                if let proofRun {
                    BlockedRunRecoveryView(run: proofRun)
                        .frame(minHeight: 520)
                        .accessibilityIdentifier("p013-recovery-view")
                }

                Button("Run Proof") {
                    Task { await runProof() }
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("p013-run-proof")

                if proofCompleted {
                    Color.clear
                        .frame(width: 1, height: 1)
                        .accessibilityIdentifier("p013-proof-complete")
                }
            }
            .padding()
        }
        .frame(minWidth: 600, minHeight: 500)
    }

    private func runProof() async {
        proofSteps = []
        proofStatus = "Running..."
        fanoutArtifactCount = 0
        aggregateFailureSummary = "Not evaluated"
        narrowestActionSummary = "Not evaluated"
        proofCompleted = false
        let harness = Proposal013AppProofHarness(
            modelContext: modelContext,
            executionService: executionService
        )

        do {
            proofSteps.append("[1/5] Launching fixture-backed proposal loop")
            let (run, packet, result) = try await harness.run()
            proofSteps.append("[2/5] Run reached \(result.terminalStatus)")
            proofSteps.append("[3/5] Fan-out artifacts persisted: \(result.fanoutArtifactCount)/4")
            proofSteps.append("[4/5] Aggregate failure captured: \(result.aggregateFailureSummary)")
            proofSteps.append("[5/5] Narrowest action: \(result.narrowestActionSummary)")

            fanoutArtifactCount = result.fanoutArtifactCount
            aggregateFailureSummary = result.aggregateFailureSummary
            narrowestActionSummary = result.narrowestActionSummary
            proofRun = run
            evidencePacket = packet
            proofStatus = result.proofStatus
            proofCompleted = true
        } catch {
            proofStatus = "FAIL — \(error.localizedDescription)"
            proofSteps.append("Harness error: \(error.localizedDescription)")
            proofCompleted = true
        }
    }
}

struct UITestProposal022EvidenceSurface: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @State private var proofRun: Run?
    @State private var proofStatus: String = "Not started"
    @State private var proofSteps: [String] = []
    @State private var refineCorpusInputCount = 0
    @State private var reviewCorpusBundlePresent = false
    @State private var scoreLiftBacklogPresent = false
    @State private var scoreLiftMergeProvenancePresent = false
    @State private var proposalFeedbackCoveragePresent = false
    @State private var unresolvedBacklogItems: [String] = []
    @State private var targetedRerunRationale = "Not evaluated"
    @State private var proofCompleted = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Proposal 022 App-Level Proof")
                    .font(.headline)
                    .accessibilityIdentifier("p022-proof-banner")

                Text(proofStatus)
                    .font(.subheadline)
                    .foregroundStyle(proofStatus.contains("PASS") ? .green : .orange)
                    .accessibilityIdentifier("p022-proof-status")
                    .accessibilityLabel(proofStatus)
                    .accessibilityValue(proofStatus)

                ForEach(Array(proofSteps.enumerated()), id: \.offset) { _, step in
                    Text(step)
                        .font(.caption.monospaced())
                }

                LabeledContent("Full Review Corpus") {
                    let label = "\(refineCorpusInputCount)/5"
                    Text(label)
                        .accessibilityIdentifier("p022-proof-refine-corpus")
                        .accessibilityLabel(label)
                        .accessibilityValue(label)
                }

                LabeledContent("Review Corpus Bundle") {
                    Text(reviewCorpusBundlePresent ? "present" : "missing")
                        .accessibilityIdentifier("p022-review-corpus-bundle-present")
                        .accessibilityLabel(reviewCorpusBundlePresent ? "present" : "missing")
                        .accessibilityValue(reviewCorpusBundlePresent ? "present" : "missing")
                }

                LabeledContent("Score Lift Backlog") {
                    Text(scoreLiftBacklogPresent ? "present" : "missing")
                        .accessibilityIdentifier("p022-score-lift-backlog-present")
                        .accessibilityLabel(scoreLiftBacklogPresent ? "present" : "missing")
                        .accessibilityValue(scoreLiftBacklogPresent ? "present" : "missing")
                }

                LabeledContent("Merge Provenance") {
                    Text(scoreLiftMergeProvenancePresent ? "present" : "missing")
                        .accessibilityIdentifier("p022-score-lift-merge-provenance-present")
                        .accessibilityLabel(scoreLiftMergeProvenancePresent ? "present" : "missing")
                        .accessibilityValue(scoreLiftMergeProvenancePresent ? "present" : "missing")
                }

                LabeledContent("Feedback Coverage") {
                    Text(proposalFeedbackCoveragePresent ? "present" : "missing")
                        .accessibilityIdentifier("p022-feedback-coverage-present")
                        .accessibilityLabel(proposalFeedbackCoveragePresent ? "present" : "missing")
                        .accessibilityValue(proposalFeedbackCoveragePresent ? "present" : "missing")
                }

                LabeledContent("Unresolved Items") {
                    Text(unresolvedBacklogItems.isEmpty ? "none" : unresolvedBacklogItems.joined(separator: ", "))
                        .accessibilityIdentifier("p022-unresolved-items")
                        .accessibilityLabel(unresolvedBacklogItems.isEmpty ? "none" : unresolvedBacklogItems.joined(separator: ", "))
                        .accessibilityValue(unresolvedBacklogItems.isEmpty ? "none" : unresolvedBacklogItems.joined(separator: ", "))
                }

                LabeledContent("Targeted Rerun Rationale") {
                    Text(targetedRerunRationale)
                        .accessibilityIdentifier("p022-proof-targeted-reviewers")
                        .accessibilityLabel(targetedRerunRationale)
                        .accessibilityValue(targetedRerunRationale)
                }

                if let proofRun {
                    GroupBox("Proof Run") {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Run ID: \(proofRun.id.uuidString)")
                            Text("Status: \(proofRun.presentationStatusLabel)")
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .accessibilityIdentifier("p022-proof-run")
                }

                Button("Run Proof") {
                    Task { await runProof() }
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("p022-run-proof")

                if proofCompleted {
                    Color.clear
                        .frame(width: 1, height: 1)
                        .accessibilityIdentifier("p022-proof-complete")
                }
            }
            .padding()
        }
        .frame(minWidth: 640, minHeight: 480)
    }

    private func runProof() async {
        proofSteps = []
        proofStatus = "Running..."
        refineCorpusInputCount = 0
        reviewCorpusBundlePresent = false
        scoreLiftBacklogPresent = false
        scoreLiftMergeProvenancePresent = false
        proposalFeedbackCoveragePresent = false
        unresolvedBacklogItems = []
        targetedRerunRationale = "Not evaluated"
        proofCompleted = false

        let harness = Proposal022AppProofHarness(
            modelContext: modelContext,
            executionService: executionService
        )

        do {
            proofSteps.append("[1/6] Launching fixture-backed proposal loop")
            let (run, summary, result) = try await harness.run()
            proofSteps.append("[2/6] Refine corpus persisted: \(result.refineCorpusInputCount)/5")
            proofSteps.append("[3/6] Review corpus bundle present: \(result.reviewCorpusBundleExists && result.reviewCorpusBundleConsumed)")
            proofSteps.append("[4/6] Score-lift backlog present: \(result.scoreLiftBacklogExists)")
            proofSteps.append("[5/6] Merge provenance present: \(result.scoreLiftBacklogMergeProvenanceExists)")
            proofSteps.append("[6/6] Targeted rerun rationale: \(summary.targetedReviewerSummary ?? "missing")")

            proofRun = run
            refineCorpusInputCount = result.refineCorpusInputCount
            reviewCorpusBundlePresent = result.reviewCorpusBundleExists && result.reviewCorpusBundleConsumed
            scoreLiftBacklogPresent = result.scoreLiftBacklogExists
            scoreLiftMergeProvenancePresent = result.scoreLiftBacklogMergeProvenanceExists
            proposalFeedbackCoveragePresent = result.proposalFeedbackCoverageExists
            unresolvedBacklogItems = result.unresolvedBacklogItemIDs
            targetedRerunRationale = result.targetedRerunRationale
            proofStatus = result.proofStatus
            proofCompleted = true
        } catch {
            proofStatus = "FAIL — \(error.localizedDescription)"
            proofSteps.append("Harness error: \(error.localizedDescription)")
            proofCompleted = true
        }
    }
}
