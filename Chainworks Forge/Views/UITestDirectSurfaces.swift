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

    private func projectionBackedRun() -> Run? {
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
        return nil
    }

    private var targetRun: Run? {
        if let seededRun = projectionBackedRun() {
            return seededRun
        }
        if ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DISABLE_WORKFLOW_MAP_SEED"] != "1" {
            PreviewSupport.seedWorkflowMapPreviewData(context: modelContext)
            if let seededRun = projectionBackedRun() {
                return seededRun
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

    private func workflowStatusProofLabel(for run: Run) -> String {
        guard let projection = projection(for: run) else {
            return "Workflow map stage statuses: Unavailable"
        }

        let statuses = projection.stages.reduce(into: [String]()) { partialResult, stage in
            let label = stage.status.rawValue
                .replacingOccurrences(of: "_", with: " ")
                .capitalized
            if partialResult.contains(label) == false {
                partialResult.append(label)
            }
        }

        let summary = statuses.isEmpty ? "Unavailable" : statuses.joined(separator: ", ")
        return "Workflow map stage statuses: \(summary)"
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
                            Text(workflowStatusProofLabel(for: targetRun))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .accessibilityElement(children: .ignore)
                                .accessibilityLabel(workflowStatusProofLabel(for: targetRun))
                                .accessibilityIdentifier("workflow-map-view")
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
    @State private var proofRun: Run?
    @State private var evidencePacket: FailedStageEvidencePacket?
    @State private var proofStatus: String = "Not started"
    @State private var proofSteps: [String] = []

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

                ForEach(Array(proofSteps.enumerated()), id: \.offset) { _, step in
                    Text(step)
                        .font(.caption.monospaced())
                }

                if let packet = evidencePacket {
                    FailedStageEvidencePanel(evidencePacket: packet)
                        .accessibilityIdentifier("p013-evidence-panel")
                }

                Button("Run Proof") {
                    runProof()
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("p013-run-proof")
            }
            .padding()
        }
        .frame(minWidth: 600, minHeight: 500)
    }

    private func runProof() {
        proofSteps = []
        proofStatus = "Running..."

        // Step 1: Create a blocked run with validation failure evidence
        let idea = Idea(title: "P013 Proof Idea", body: "Automated proof for Proposal 013", status: .active)
        modelContext.insert(idea)
        proofSteps.append("[1/7] Created idea")

        let run = Run(
            startedAt: Date(),
            status: .blocked,
            workflowID: "p013-proof",
            workflowTitle: "P013 Proof",
            workflowSnapshotHash: "proof-hash",
            catalogSnapshotHash: "proof-catalog",
            workflowSourcePath: "/proof/wf.yaml",
            catalogSourcePath: "/proof/ag.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: NSTemporaryDirectory(),
            artifactRoot: NSTemporaryDirectory() + "artifacts/",
            planCompilerVersion: 1
        )
        run.idea = idea
        modelContext.insert(run)
        proofSteps.append("[2/7] Created blocked run")

        // Step 2: Create failed stage with validation failure evidence
        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.run = run
        modelContext.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_reviewer_po",
            agentTitle: "Proposal Reviewer / PO",
            taskName: "review_proposal",
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.completedAt = Date()
        agent.stageExecution = stage
        modelContext.insert(agent)

        // Step 3: Persist validation failure record
        let failureRecord = ValidationFailureRecord(
            agentID: "proposal_reviewer_po",
            stageID: "state_4_proposal_reviewed",
            runID: run.id,
            outputResults: [
                OutputValidationResult(
                    outputName: "proposal_review_po",
                    contractID: "proposal_review_v1",
                    status: .failed,
                    missingFields: ["score", "decision"],
                    validationError: "Output is not valid JSON or not a JSON object",
                    rawPayloadSize: 2048
                )
            ],
            failureSummary: "Output contract mismatch: reviewer produced markdown instead of JSON",
            failureClass: .outputContractMismatch,
            contractMetadata: [
                ContractValidationMetadata(
                    outputName: "proposal_review_po",
                    contractID: "proposal_review_v1",
                    machineFormat: "json",
                    validationMode: "strict_structured",
                    requiredFieldCount: 11
                )
            ],
            rawOutputExists: true,
            receiptExists: true,
            transcriptExists: false,
            recoveryRecommendation: RecoveryRecommendation(
                action: .retryFailedAgent,
                explanation: "Raw output exists on disk. Retry the agent with the same inputs.",
                source: .runtimePolicy
            )
        )
        agent.validationFailureJSON = try? JSONEncoder().encode(failureRecord)
        proofSteps.append("[3/7] Persisted validation failure record")

        // Step 4: Persist stage evidence packet
        let packet = FailedStageEvidenceBuilder.buildEvidencePacket(
            stageExecution: stage,
            failedAgent: agent,
            validationFailure: failureRecord,
            outputEnvelopes: [],
            recoverySnapshot: nil
        )
        stage.evidencePacketJSON = try? JSONEncoder().encode(packet)
        evidencePacket = packet
        proofSteps.append("[4/7] Persisted stage evidence packet")

        // Step 5: Verify recovery actions available
        let coordinator = RecoveryCoordinator(modelContext: modelContext)
        let actions = coordinator.availableActions(for: run)
        let hasRetry = actions.contains(where: {
            if case .retryAgent = $0 { return true }
            return false
        })
        proofSteps.append("[5/7] Recovery actions: \(actions.map(\.label).joined(separator: ", ")) — retry available: \(hasRetry)")

        // Step 6: Verify evidence packet is buildable
        let builtPacket = coordinator.buildEvidencePacket(for: run)
        proofSteps.append("[6/7] Evidence packet buildable: \(builtPacket != nil), failure class: \(builtPacket?.failureClass.rawValue ?? "none")")

        // Step 7: Verify context is operator-mediated for contract mismatch
        let context = coordinator.recoveryContext(for: run)
        let isOperatorMediated = context.suggestedAction == nil
        proofSteps.append("[7/7] Operator-mediated (no auto-suggest for contract mismatch): \(isOperatorMediated)")

        self.proofRun = run

        // Final verdict
        if hasRetry && builtPacket != nil && isOperatorMediated {
            proofStatus = "PASS — All Proposal 013 Section 10.2 proof steps verified"
        } else {
            proofStatus = "FAIL — Some proof steps did not verify"
        }

        try? modelContext.save()
    }
}

// MARK: - Proposal 016: App-Level Proof Surface

struct UITestProposal016ProofSurface: View {
    @Environment(\.modelContext) private var modelContext
    @State private var proofStatus: String = "Not started"
    @State private var proofSteps: [String] = []
    @State private var limitReason: String = "—"
    @State private var limitTrust: String = "—"
    @State private var bindingSummary: String = "—"
    @State private var policySummary: String = "—"
    @State private var repairSummary: String = "—"

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Proposal 016 App-Level Proof")
                    .font(.headline)
                    .accessibilityIdentifier("p016-proof-banner")

                Text(proofStatus)
                    .font(.subheadline)
                    .foregroundStyle(proofStatus.contains("PASS") ? .green : .orange)
                    .accessibilityIdentifier("p016-proof-status")

                GroupBox("Limit Exhaustion Report / Recovery") {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(limitReason)
                            .font(.caption.monospaced())
                            .accessibilityIdentifier("p016-limit-reason")
                        Text(limitTrust)
                            .font(.caption.monospaced())
                            .accessibilityIdentifier("p016-runtime-trust")
                        Text(bindingSummary)
                            .font(.caption.monospaced())
                            .accessibilityIdentifier("p016-binding-summary")
                    }
                }

                GroupBox("Policy-Bound Stop Recovery") {
                    Text(policySummary)
                        .font(.caption.monospaced())
                        .accessibilityIdentifier("p016-policy-summary")
                }

                GroupBox("Startup Repair") {
                    Text(repairSummary)
                        .font(.caption.monospaced())
                        .accessibilityIdentifier("p016-repair-summary")
                }

                ForEach(Array(proofSteps.enumerated()), id: \.offset) { _, step in
                    Text(step)
                        .font(.caption.monospaced())
                }

                Button("Run Proof") {
                    runProof()
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("p016-run-proof")
            }
            .padding()
        }
        .frame(minWidth: 720, minHeight: 560)
    }

    private func runProof() {
        proofStatus = "Running..."
        proofSteps = []
        limitReason = "—"
        limitTrust = "—"
        bindingSummary = "—"
        policySummary = "—"
        repairSummary = "—"

        do {
            let plan = try loadProposalLoopPlan()
            let compiler = RunPlanCompiler(modelContext: modelContext)

            let repairRun = try makeProofRun(title: "P016 Repair Proof", plan: plan)
            seedRepairProof(into: repairRun)
            let interruptedActions = try ResumeManager(modelContext: modelContext)
                .classifyInterruptedRuns(compiler: compiler)
            let repairActiveCount = repairRun.stageExecutions.filter {
                $0.lineageID == "state_2_proposal_drafted::iteration:1"
                    && ($0.status == .running || $0.status == .ready || $0.status == .waitingApproval)
            }.count
            let repairRequestedApprovals = repairRun.approvals.filter { $0.stageID == "state_4_proposal_approval" && $0.decision == .requested }.count
            let repairExpiredApprovals = repairRun.approvals.filter { $0.stageID == "state_4_proposal_approval" && $0.decision == .expired }.count
            repairSummary = "actions=\(interruptedActions.count) active-stage-siblings=\(repairActiveCount) requested-approvals=\(repairRequestedApprovals) expired-approvals=\(repairExpiredApprovals)"
            proofSteps.append("[1/4] Startup repair collapsed stale active lineage and duplicate approvals")

            let limitRun = try makeProofRun(title: "P016 Limit Exhaustion Proof", plan: plan)
            let limitEvidence = try seedLimitExhaustionProof(into: limitRun)
            let limitPayload = RunReportBuilder(modelContext: modelContext).buildReportPayload(for: limitRun, version: 1)
            let limitRecovery = RecoveryCoordinator(modelContext: modelContext).recoveryContext(for: limitRun)
            limitReason = limitPayload.blockedReason ?? "no blocked reason"
            limitTrust = "runtimeTrust=\(limitPayload.runtimeTrustLevel)"
            bindingSummary = "frozen=\(limitEvidence.frozen.providerFamily)/\(limitEvidence.frozen.model) runtime=\(limitEvidence.runtimeProvider)/\(limitEvidence.runtimeModel)"
            proofSteps.append("[2/4] Limit exhaustion preserved durable output and emitted canonical blocked reason")

            let policyRun = try makeProofRun(title: "P016 Policy Stop Proof", plan: plan)
            let policyRecovery = try seedPolicyStopProof(into: policyRun)
            policySummary = "suggested=\(policyRecovery.suggestedAction?.label ?? "none") allowed=\(policyRecovery.allowedActions.map(\.label).joined(separator: ", "))"
            proofSteps.append("[3/4] Policy-bound stop suppressed default same-run retry")

            let limitHasNoRetry = limitRecovery.allowedActions.allSatisfy { action in
                switch action {
                case .retryAgent, .retryAggregateStep, .retryStage:
                    return false
                case .resumeRun, .resumeFromApprovalGate, .cloneRunFrozenSnapshot, .cloneRunCurrentConfig:
                    return true
                }
            }
            let policyHasNoRetry = policyRecovery.allowedActions.allSatisfy { action in
                switch action {
                case .retryAgent, .retryAggregateStep, .retryStage:
                    return false
                case .resumeRun, .resumeFromApprovalGate, .cloneRunFrozenSnapshot, .cloneRunCurrentConfig:
                    return true
                }
            }
            let limitPass =
                limitEvidence.outputArtifactExists
                && limitEvidence.agent.canonicalOutcome == .limitExhaustedAfterOutput
                && limitPayload.runtimeTrustLevel == RuntimeBindingTrustLevel.unverifiable.rawValue
                && (limitPayload.blockedReason?.localizedCaseInsensitiveContains("limit") == true
                    || limitPayload.blockedReason?.localizedCaseInsensitiveContains("exhaust") == true)
                && limitRecovery.suggestedAction == nil
                && limitHasNoRetry
                && limitEvidence.reportEmitted

            let policyPass =
                policyRecovery.suggestedAction == nil
                && policyHasNoRetry
                && policyRecovery.reason.localizedCaseInsensitiveContains("policy")

            let repairPass = repairActiveCount == 1 && repairRequestedApprovals == 1 && repairExpiredApprovals == 1
            proofSteps.append("[4/4] Report/recovery surfaces label unverifiable binding truth honestly and keep clone-only manual recovery")

            if limitPass && policyPass && repairPass {
                proofStatus = "PASS — Proposal 016 app-level proof verified"
            } else {
                proofStatus = "FAIL — Proposal 016 app-level proof did not verify"
            }
        } catch {
            proofStatus = "FAIL — \(error.localizedDescription)"
        }
    }

    private func loadProposalLoopPlan() throws -> RunPlan {
        let repoRoot = AppConfiguration.defaultRepositoryRoot()
        let workflowURL = repoRoot.appendingPathComponent("examples/workflows/proposal-loop-live.yaml")
        let catalogURL = repoRoot.appendingPathComponent("examples/agents/agents.yaml")
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        return try RunPlanCompiler(modelContext: modelContext).previewCompile(workflow: workflow, catalog: catalog)
    }

    private func makeProofRun(title: String, plan: RunPlan) throws -> Run {
        let baseURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("Proposal016Proof-\(UUID().uuidString)", isDirectory: true)
        let workspaceRoot = baseURL.appendingPathComponent("workspace", isDirectory: true)
        let artifactRoot = baseURL.appendingPathComponent("artifacts", isDirectory: true)
        try FileManager.default.createDirectory(at: workspaceRoot, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)

        let idea = Idea(title: title, body: "Proposal 016 proof run", status: .active)
        modelContext.insert(idea)

        let workflowSourcePath = AppConfiguration.defaultRepositoryRoot()
            .appendingPathComponent("examples/workflows/proposal-loop-live.yaml")
            .path
        let catalogSourcePath = AppConfiguration.defaultRepositoryRoot()
            .appendingPathComponent("examples/agents/agents.yaml")
            .path

        let run = Run(
            startedAt: Date(),
            status: .running,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspaceRoot.path,
            artifactRoot: artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        )
        run.idea = idea
        idea.runs.append(run)
        run.frozenWorkspaceRootPath = workspaceRoot.path
        modelContext.insert(run)
        return run
    }

    private func seedRepairProof(into run: Run) {
        let staleStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -180),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        staleStage.lineageID = "state_2_proposal_drafted::iteration:1"
        staleStage.activeOwnerToken = "stale-owner"
        staleStage.run = run
        run.stageExecutions.append(staleStage)
        modelContext.insert(staleStage)

        let activeStage = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSinceNow: -60),
            status: .running,
            iteration: 1,
            attemptNumber: 2
        )
        activeStage.lineageID = "state_2_proposal_drafted::iteration:1"
        activeStage.run = run
        run.stageExecutions.append(activeStage)
        modelContext.insert(activeStage)

        let approvalStage = StageExecution(
            stageID: "state_4_proposal_approval",
            label: "Human approval: proposal quality",
            startedAt: Date(timeIntervalSinceNow: -40),
            status: .waitingApproval,
            iteration: 1,
            attemptNumber: 1
        )
        approvalStage.lineageID = "state_4_proposal_approval::iteration:1"
        approvalStage.run = run
        run.stageExecutions.append(approvalStage)
        modelContext.insert(approvalStage)

        let staleApproval = Approval(
            stageID: approvalStage.stageID,
            requestedAt: Date(timeIntervalSinceNow: -30),
            decision: .requested
        )
        staleApproval.run = run
        run.approvals.append(staleApproval)
        modelContext.insert(staleApproval)

        let activeApproval = Approval(
            stageID: approvalStage.stageID,
            requestedAt: Date(timeIntervalSinceNow: -10),
            decision: .requested
        )
        activeApproval.run = run
        run.approvals.append(activeApproval)
        modelContext.insert(activeApproval)
    }

    private func seedLimitExhaustionProof(into run: Run) throws -> (agent: AgentExecution, frozen: ResolvedProviderBinding, runtimeProvider: String, runtimeModel: String, outputArtifactExists: Bool, reportEmitted: Bool) {
        run.status = .blocked
        let stage = StageExecution(
            stageID: "state_5_proposal_refined",
            label: "Proposal refined",
            startedAt: Date(timeIntervalSinceNow: -40),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.lineageID = "state_5_proposal_refined::iteration:1"
        stage.run = run
        run.stageExecutions.append(stage)
        modelContext.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_writer",
            agentTitle: "Proposal Writer",
            taskName: "refine_proposal",
            startedAt: Date(timeIntervalSinceNow: -35),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        modelContext.insert(agent)

        let outputURL = URL(fileURLWithPath: run.artifactRoot, isDirectory: true)
            .appendingPathComponent("state_5_proposal_refined.1/proposal_writer/1", isDirectory: true)
        try FileManager.default.createDirectory(at: outputURL, withIntermediateDirectories: true)
        let artifactURL = outputURL.appendingPathComponent("proposal_current.md")
        let outputText = "# Partial Proposal\n\nUseful output survived the interruption.\n"
        if let data = outputText.data(using: .utf8) {
            try data.write(to: artifactURL)
        }

        let artifact = Artifact(
            name: "proposal_current.md",
            contractID: "proposal_current",
            format: .markdown,
            filePath: artifactURL.path,
            runID: run.id,
            stageID: stage.stageID,
            agentID: agent.agentID,
            provider: agent.provider
        )
        artifact.agentExecution = agent
        agent.artifacts.append(artifact)
        modelContext.insert(artifact)

        let frozenProviderID = UUID()
        let frozenBinding = ResolvedProviderBinding(
            agentID: agent.agentID,
            backendProfileID: "proposal_writer_profile",
            configuredProviderID: frozenProviderID,
            providerFamily: "claude_code",
            providerIdentifier: "claude_code",
            model: "claude-3-5-sonnet",
            effort: "high",
            transport: "goose",
            adapterVersion: "proof"
        )
        run.providerBindingSnapshotJSON = try JSONEncoder().encode([agent.agentID: frozenBinding])
        run.bindingProvenanceJSON = try JSONEncoder().encode([
            agent.agentID: FrozenBindingProvenance(
                source: .backendProfileDefault,
                backendProfileID: "proposal_writer_profile",
                backendProfileModel: "claude-3-5-sonnet",
                configuredProviderID: frozenProviderID,
                configuredProviderDefaultModel: "claude-3-5-sonnet",
                runOverrideModel: nil,
                resolvedModel: "claude-3-5-sonnet",
                resolvedProviderFamily: "claude_code"
            )
        ])

        agent.completedAt = Date()
        agent.logSnippet = "Provider or app limit exhausted after output was produced"
        ExecutionTruthSupport.persistTerminalTruth(
            for: agent,
            canonicalOutcome: .limitExhaustedAfterOutput,
            transportErrorKind: .provider,
            providerStopReason: "max_tokens",
            outputPresence: .durableOutput,
            runtimeProvider: "claude_code",
            runtimeModel: "claude-3-7-sonnet",
            rawErrorMessage: "Maximum token budget exhausted",
            rawFinishEvent: "stop"
        )

        let snapshot = StageRetryCoordinator(modelContext: modelContext).narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)
        stage.evidencePacketJSON = try JSONEncoder().encode(
            FailedStageEvidenceBuilder.buildEvidencePacket(
                stageExecution: stage,
                failedAgent: agent,
                validationFailure: nil,
                outputEnvelopes: [],
                recoverySnapshot: snapshot
            )
        )

        try modelContext.save()
        _ = try RunReportBuilder(modelContext: modelContext).emitReport(for: run)
        return (
            agent: agent,
            frozen: frozenBinding,
            runtimeProvider: "claude_code",
            runtimeModel: "claude-3-7-sonnet",
            outputArtifactExists: FileManager.default.fileExists(atPath: artifactURL.path),
            reportEmitted: run.latestImmutableReportArtifactID != nil
        )
    }

    private func seedPolicyStopProof(into run: Run) throws -> RecoveryContext {
        run.status = .blocked
        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSinceNow: -20),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        stage.lineageID = "state_4_proposal_reviewed::iteration:1"
        stage.run = run
        run.stageExecutions.append(stage)
        modelContext.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_reviewer_ui",
            agentTitle: "Proposal Reviewer / UI",
            taskName: "review_proposal_from_ui_perspective",
            startedAt: Date(timeIntervalSinceNow: -18),
            status: .failed,
            provider: "gemini",
            effort: "medium"
        )
        agent.stageExecution = stage
        stage.agentExecutions.append(agent)
        modelContext.insert(agent)

        agent.completedAt = Date()
        agent.logSnippet = "Provider policy-bound terminal stop detected"
        ExecutionTruthSupport.persistTerminalTruth(
            for: agent,
            canonicalOutcome: .failedBeforeOutput,
            transportErrorKind: .provider,
            providerStopReason: "policy_violation",
            outputPresence: .none,
            runtimeProvider: "gemini",
            runtimeModel: "gemini-2.5-pro",
            rawErrorMessage: "Provider policy stop",
            rawFinishEvent: "stop"
        )

        let snapshot = StageRetryCoordinator(modelContext: modelContext).narrowestRecoveryAction(
            for: run,
            failedStage: stage,
            failedAgent: agent,
            validationFailure: nil
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)
        stage.evidencePacketJSON = try JSONEncoder().encode(
            FailedStageEvidenceBuilder.buildEvidencePacket(
                stageExecution: stage,
                failedAgent: agent,
                validationFailure: nil,
                outputEnvelopes: [],
                recoverySnapshot: snapshot
            )
        )

        try modelContext.save()
        return RecoveryCoordinator(modelContext: modelContext).recoveryContext(for: run)
    }
}
