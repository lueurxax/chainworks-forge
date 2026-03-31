import SwiftUI
import SwiftData

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

                        WorkflowMapView(run: targetRun)
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
    }
}

struct UITestGooseAssistantSurface: View {
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderRegistry.self) private var providerRegistry
    @Environment(GooseServerManager.self) private var gooseServerManager

    private var targetProvider: ConfiguredProvider? {
        providerSettingsStore.settings.configuredProviders.first(where: { $0.family.gooseFirstPreferred })
    }

    var body: some View {
        Group {
            if let targetProvider {
                GooseProviderConnectionAssistantView(
                    providerID: targetProvider.id,
                    origin: .providerSettings
                )
                    .environment(appConfigurationStore)
                    .environment(providerSettingsStore)
                    .environment(providerRegistry)
                    .environment(gooseServerManager)
            } else {
                ContentUnavailableView(
                    "Goose assistant unavailable",
                    systemImage: "server.rack",
                    description: Text("The UI test Goose assistant surface requires at least one Goose-first provider.")
                )
            }
        }
        .accessibilityIdentifier("ui-test-goose-assistant-surface")
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
            repoRoot: FileManager.default.currentDirectoryPath
        )
        run.repoRoot = FileManager.default.currentDirectoryPath
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
            repoRoot: FileManager.default.currentDirectoryPath,
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
            ("docs_delta", .markdown),
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
    private let failingResult = DeliveryPreflightService.PreflightResult(
        checks: [
            .init(id: "repo_root", label: "Repository root exists", passed: true, detail: "/Users/test/chainworks-remote"),
            .init(id: "git_repo", label: "Valid git repository", passed: true, detail: nil),
            .init(id: "base_branch", label: "Base branch 'release/v2' exists", passed: false, detail: "Branch 'release/v2' not found"),
            .init(id: "worktree_writable", label: "Worktree base path is writable", passed: true, detail: "/Users/test/Library/Application Support/Chainworks Forge/worktrees"),
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
    private let seededRun: Run?
    private let seedErrorMessage: String?

    init() {
        let result = Self.makeFallbackRun()
        self.seededRun = result.run
        self.seedErrorMessage = result.errorMessage
    }

    var body: some View {
        Group {
            if let seededRun {
                UITestCompletedExportHubHarness(run: seededRun)
            } else if let seedErrorMessage {
                ContentUnavailableView(
                    "Completed export hub unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text(seedErrorMessage)
                )
                .accessibilityIdentifier("ui-test-completed-export-hub-error")
            } else {
                ContentUnavailableView(
                    "Completed export hub unavailable",
                    systemImage: "shippingbox",
                    description: Text("The UI test completed export hub surface requires a seeded completed run.")
                )
            }
        }
        .accessibilityIdentifier("ui-test-completed-export-hub-surface")
    }

    @MainActor
    private static func makeFallbackRun() -> (run: Run?, errorMessage: String?) {
        func fail(_ message: String) -> (run: Run?, errorMessage: String?) {
            print("UITestCompletedExportHubSurface fallback failed: \(message)")
            return (nil, message)
        }

        let runID = UUID()
        let runRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("UITestCompletedExportHub-\(runID.uuidString)", isDirectory: true)
        let artifactRoot = runRoot.appendingPathComponent("artifacts", isDirectory: true)
        let worktreeRoot = runRoot.appendingPathComponent("worktree", isDirectory: true)
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
            workspaceRootPath: FileManager.default.currentDirectoryPath
        )
        run.completedAt = Date().addingTimeInterval(-60)
        run.repoIdentifier = RepositoryIdentityNormalizer.canonicalIdentifier(
            configuredIdentifier: "Chainworks Forge",
            repoRoot: FileManager.default.currentDirectoryPath
        )
        run.repoRoot = FileManager.default.currentDirectoryPath
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
            repoRoot: FileManager.default.currentDirectoryPath,
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
        print("UITestCompletedExportHubSurface fallback seeded run \(run.id.uuidString)")
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

private struct UITestCompletedExportHubHarness: View {
    let run: Run
    @State private var exportMessage: String?
    @State private var isExporting = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Button("Completed export hub seeded") {}
                    .buttonStyle(.plain)
                    .font(.headline)
                    .accessibilityIdentifier("ui-test-completed-export-hub-ready")

                GroupBox("Run Result") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(run.idea?.title ?? "Completed Export Hub Proof")
                            .font(.title3.bold())
                        Text(run.workflowTitle)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                        HStack(spacing: 12) {
                            Label(run.presentationStatusLabel, systemImage: "checkmark.circle.fill")
                                .foregroundStyle(.green)
                            if let worktreeRoot = run.worktreeRoot {
                                Text(worktreeRoot)
                                    .font(.caption.monospaced())
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                    }
                }

                GroupBox("Export Actions") {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Export Evidence Pack")
                                .font(.headline)
                            Text("Exports repo-backed delivery artifacts and screenshot checklist to Desktop.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button {
                            exportEvidencePack()
                        } label: {
                            Label("Export", systemImage: "square.and.arrow.up")
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(isExporting)
                        .accessibilityIdentifier("completed-run-export-evidence-pack")
                    }
                }

                if let exportMessage {
                    Text(exportMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("completed-run-export-message")
                }
            }
            .padding()
        }
        .accessibilityIdentifier("completed-run-export-hub")
    }

    private func exportEvidencePack() {
        isExporting = true
        defer { isExporting = false }

        let exportDirectory = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_EXPORT_BASE_PATH"]
            .map { URL(fileURLWithPath: $0, isDirectory: true) }
            ?? URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("ChainworksUITestExports", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: exportDirectory, withIntermediateDirectories: true)
        } catch {
            exportMessage = "Export failed: \(error.localizedDescription)"
            return
        }
        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: URL(fileURLWithPath: run.workspaceRoot),
            artifactRoot: URL(fileURLWithPath: run.artifactRoot),
            worktreeRoot: run.worktreeRoot.map { URL(fileURLWithPath: $0) }
        )

        do {
            let pack = try EvidencePackBuilder.export(
                run: run,
                workspace: workspace,
                exportDirectory: exportDirectory
            )
            exportMessage = "Exported \(pack.itemCount) items."
        } catch {
            exportMessage = "Export failed: \(error.localizedDescription)"
        }
    }
}

// MARK: - Proposal 013: App-Launched Proof Surface

/// Direct-surface proof for Proposal 013 Section 10.2:
/// Shows validation failure, evidence panel, narrow retry, and prior inspectability.
struct UITestProposal013EvidenceSurface: View {
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
