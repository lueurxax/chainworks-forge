import SwiftUI
import SwiftData

// MARK: - CompletedRunExportHub (Proposal 008 — §7.3, ARCH-086)

/// Terminal export subroute for completed repo-backed runs.
/// Shell-owned subview — NOT a parallel top-level destination.
///
/// Visual hierarchy per §7.3:
/// - TOP: Dominant summary block (run result, elapsed time, total cost, evidence-pack status)
/// - NEXT: Sign-off summary block (if benchmark run)
/// - DISCLOSURE: Receipt breakdowns per-stage/per-agent
/// - FOOTER: Export actions
struct CompletedRunExportHub: View {
    let run: Run
    @Environment(\.modelContext) private var modelContext
    @State private var allArtifacts: [Artifact] = []
    @State private var evidencePackStatus: EvidencePackStatus = .missing
    @State private var exportMessage: String?
    @State private var isExporting = false
    @State private var signOffSnapshot: MVPSignOffDecisionSnapshot?
    @State private var selectedArtifact: Artifact?
    @State private var dataLoadWarning: String?

    private var stageSnapshots: [RunStageSnapshot] {
        RunStageSnapshotLoader.load(for: run, modelContext: modelContext)
    }

    /// Evidence-pack lifecycle status per §7.5.
    enum EvidencePackStatus: String {
        case missing = "Missing"
        case inProgress = "In Progress"
        case ready = "Ready"
        case exported = "Exported"

        var icon: String {
            switch self {
            case .missing: return "xmark.circle"
            case .inProgress: return "arrow.clockwise.circle"
            case .ready: return "checkmark.circle"
            case .exported: return "checkmark.seal.fill"
            }
        }

        var color: Color {
            switch self {
            case .missing: return .red
            case .inProgress: return .orange
            case .ready: return .green
            case .exported: return .blue
            }
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // TOP: Dominant summary block
                resultSummarySection

                if let dataLoadWarning {
                    Label(dataLoadWarning, systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .accessibilityIdentifier("completed-run-export-load-warning")
                }

                Divider()

                // NEXT: Sign-off summary (if benchmark run)
                if signOffSnapshot != nil {
                    signOffSummarySection
                    Divider()
                }

                // Final report link
                finalReportSection

                // Repo-backed continuity affordances
                repoBackedContinuitySection

                Divider()

                // Provider and delivery receipts
                receiptOverviewSection

                // DISCLOSURE: Per-stage/per-agent receipt breakdown
                perStageReceiptBreakdown

                Divider()

                // FOOTER: Export actions
                exportActionsSection

                // Export feedback with retry on failure (Proposal 008 REQ-012)
                if let exportMessage {
                    HStack {
                        Text(exportMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        if exportMessage.hasPrefix("Export failed") {
                            Button("Retry") {
                                exportEvidencePack()
                            }
                            .font(.caption)
                            .buttonStyle(.bordered)
                            .accessibilityIdentifier("export-retry-button")
                        }
                    }
                    .transition(.opacity)
                    .accessibilityIdentifier("completed-run-export-message")
                }
            }
            .padding()
        }
        .navigationTitle("Export Hub")
        .accessibilityIdentifier("completed-run-export-hub")
        .sheet(item: $selectedArtifact) { artifact in
            NavigationStack {
                ArtifactInspectorView(artifact: artifact, run: run)
            }
            .frame(minWidth: 960, minHeight: 640)
        }
        .task {
            loadRunData()
        }
    }

    // MARK: - Result Summary (§7.3 — Dominant block)

    private var resultSummarySection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                // Run identity
                HStack(spacing: 10) {
                    Image(systemName: resultIcon)
                        .font(.largeTitle)
                        .foregroundStyle(resultColor)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(run.idea?.title ?? "Unknown Idea")
                            .font(.title2.bold())
                        Text(run.workflowTitle)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    resultBadge
                }

                Divider()

                // Key metrics grid
                HStack(spacing: 0) {
                    metricCell(
                        label: "Elapsed Time",
                        value: formattedElapsed,
                        icon: "clock.fill",
                        tint: .blue
                    )
                    Divider().frame(height: 44)
                    metricCell(
                        label: "Total Cost",
                        value: formattedCost,
                        icon: "dollarsign.circle.fill",
                        tint: .green
                    )
                    Divider().frame(height: 44)
                    metricCell(
                        label: "Stages",
                        value: "\(completedStageCount)/\(stageSnapshots.count)",
                        icon: "rectangle.stack.fill",
                        tint: .purple
                    )
                    Divider().frame(height: 44)
                    metricCell(
                        label: "Evidence Pack",
                        value: evidencePackStatus.rawValue,
                        icon: evidencePackStatus.icon,
                        tint: evidencePackStatus.color
                    )
                }

                // Trust and parent badges
                HStack(spacing: 8) {
                    RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
                    ParentIdeaArchiveBadge(title: "Parent idea", idea: run.idea)
                    if run.deliveryConfigurationJSON != nil {
                        Label("Repo-backed", systemImage: "arrow.triangle.branch")
                            .font(.caption2.bold())
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Color.indigo.opacity(0.14), in: Capsule())
                            .foregroundStyle(.indigo)
                    }
                }
            }
        } label: {
            Label("Run Result", systemImage: "flag.checkered")
        }
    }

    private func metricCell(label: String, value: String, icon: String, tint: Color) -> some View {
        VStack(spacing: 4) {
            Image(systemName: icon)
                .font(.title3)
                .foregroundStyle(tint)
            Text(value)
                .font(.headline.monospaced())
            Text(label)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Sign-Off Summary (§7.3 — Benchmark runs)

    @ViewBuilder
    private var signOffSummarySection: some View {
        if let snapshot = signOffSnapshot {
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: snapshot.decision == .go ? "checkmark.seal.fill" : "hand.raised.fill")
                            .font(.title2)
                            .foregroundStyle(snapshot.decision == .go ? .green : .red)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(snapshot.decision == .go ? "GO" : "HOLD")
                                .font(.title3.bold())
                                .foregroundStyle(snapshot.decision == .go ? .green : .red)
                            Text("MVP Sign-Off Decision")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 2) {
                            Text("Evaluator v\(snapshot.evaluatorVersion)")
                                .font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                            Text(snapshot.evaluatedAt.formatted(.dateTime))
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                    }

                    HStack(spacing: 16) {
                        LabeledContent("Pairs") {
                            Text("\(snapshot.pairCount)")
                                .font(.caption.monospaced())
                        }
                        LabeledContent("Happy Path") {
                            Text("\(snapshot.happyPathCount)")
                                .font(.caption.monospaced())
                        }
                        LabeledContent("Recovered") {
                            Text("\(snapshot.recoveredCount)")
                                .font(.caption.monospaced())
                        }
                    }
                    .font(.caption)

                    if let median = snapshot.medianImprovementPercent {
                        HStack {
                            Text("Median Improvement:")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text(String(format: "%.1f%%", median))
                                .font(.caption.monospaced().bold())
                                .foregroundStyle(median >= 0 ? .green : .red)
                        }
                    }
                }
            } label: {
                Label("Sign-Off Summary", systemImage: "checkmark.seal")
            }
        }
    }

    // MARK: - Final Report Link

    private var finalReportSection: some View {
        GroupBox {
            HStack(spacing: 12) {
                Image(systemName: "doc.text.fill")
                    .font(.title3)
                    .foregroundStyle(.blue)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Final Run Report")
                        .font(.subheadline.bold())
                    if run.latestImmutableReportArtifactID != nil {
                        Text("v\(run.latestReportVersion) - Immutable history available")
                            .font(.caption)
                            .foregroundStyle(.green)
                    } else if run.latestSummaryArtifactID != nil {
                        Text("Latest summary available (no immutable report)")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    } else {
                        Text("No report artifacts found")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
            }
        } label: {
            Label("Report", systemImage: "doc.text")
        }
    }

    // MARK: - Repo-Backed Continuity

    private var repoBackedContinuitySection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                continuityWorktreeRow

                Divider()

                continuityArtifactRow(
                    title: "Open Release Manifest",
                    subtitle: releaseManifestArtifact.map { artifactContinuitySubtitle(for: $0) }
                        ?? "Release manifest not captured",
                    systemImage: "doc.text",
                    artifact: releaseManifestArtifact,
                    accessibilityIdentifier: "completed-run-open-release_manifest"
                )

                Divider()

                continuityArtifactRow(
                    title: "Open Git Push Receipt",
                    subtitle: gitPushReceiptArtifact.map { artifactContinuitySubtitle(for: $0) }
                        ?? "Git push receipt not captured",
                    systemImage: "arrow.up.right.circle",
                    artifact: gitPushReceiptArtifact,
                    accessibilityIdentifier: "completed-run-open-git_push_receipt"
                )

                Divider()

                continuityArtifactRow(
                    title: "Open Upload Receipt",
                    subtitle: uploadReceiptArtifact.map { artifactContinuitySubtitle(for: $0) }
                        ?? "Upload receipt not captured",
                    systemImage: "square.and.arrow.up.circle",
                    artifact: uploadReceiptArtifact,
                    accessibilityIdentifier: "completed-run-open-connect_upload_receipt"
                )
            }
        } label: {
            Label("Repo-Backed Continuity", systemImage: "arrow.triangle.branch")
        }
    }

    private var continuityWorktreeRow: some View {
        HStack(spacing: 12) {
            let worktreeRoot = run.worktreeRoot ?? ""
            Image(systemName: "folder")
                .font(.title3)
                .foregroundStyle(.orange)
            VStack(alignment: .leading, spacing: 2) {
                Text("Reveal Worktree")
                    .font(.subheadline.bold())
                Text(worktreeRoot.isEmpty ? "No worktree captured for this run." : worktreeRoot)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer()
            Button {
                revealWorktree()
            } label: {
                Label("Reveal", systemImage: "folder")
            }
            .buttonStyle(.bordered)
            .disabled(worktreeRoot.isEmpty)
            .accessibilityIdentifier("completed-run-open-worktree")
            Button {
                ArtifactPathClipboard.copy(path: worktreeRoot)
            } label: {
                Label("Copy Path", systemImage: "doc.on.clipboard")
            }
            .buttonStyle(.bordered)
            .disabled(worktreeRoot.isEmpty)
            .accessibilityIdentifier("completed-run-copy-worktree")
        }
    }

    private func continuityArtifactRow(
        title: String,
        subtitle: String,
        systemImage: String,
        artifact: Artifact?,
        accessibilityIdentifier: String
    ) -> some View {
        HStack(spacing: 12) {
            Image(systemName: systemImage)
                .font(.title3)
                .foregroundStyle(artifact == nil ? Color.secondary : Color.blue)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.subheadline.bold())
                Text(subtitle)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer()
            Button {
                if let artifact {
                    selectedArtifact = artifact
                }
            } label: {
                Label("Open", systemImage: "doc.text.magnifyingglass")
            }
            .buttonStyle(.bordered)
            .disabled(artifact == nil)
            .accessibilityIdentifier(accessibilityIdentifier)
            Button {
                if let artifact {
                    ArtifactPathClipboard.copy(path: artifact.filePath)
                }
            } label: {
                Label("Copy Path", systemImage: "doc.on.clipboard")
            }
            .buttonStyle(.bordered)
            .disabled(artifact == nil)
            .accessibilityIdentifier(accessibilityIdentifier.replacingOccurrences(of: "-open-", with: "-copy-"))
        }
    }

    // MARK: - Receipt Overview

    private var receiptOverviewSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                let receiptArtifacts = allArtifacts.filter {
                    $0.name.localizedCaseInsensitiveContains("receipt")
                }
                let providerReceipts = stageSnapshots
                    .flatMap(\.agentExecutions)
                    .filter { $0.providerReceiptPresent }

                HStack(spacing: 16) {
                    HStack(spacing: 4) {
                        Image(systemName: "doc.seal.fill")
                            .foregroundStyle(.green)
                        Text("\(receiptArtifacts.count) delivery receipt(s)")
                            .font(.caption)
                    }
                    HStack(spacing: 4) {
                        Image(systemName: "server.rack")
                            .foregroundStyle(.blue)
                        Text("\(providerReceipts.count) provider receipt(s)")
                            .font(.caption)
                    }
                }

                if receiptArtifacts.isEmpty && providerReceipts.isEmpty {
                    HStack {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.secondary)
                        Text("No receipts captured for this run.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                } else {
                    ForEach(receiptArtifacts) { artifact in
                        HStack(spacing: 6) {
                            Image(systemName: "doc.seal")
                                .font(.caption2)
                                .foregroundStyle(.green)
                            Text(artifact.name)
                                .font(.caption.monospaced())
                            Spacer()
                            Text(artifact.stageID)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        } label: {
            Label("Receipts", systemImage: "doc.seal")
        }
    }

    // MARK: - Per-Stage/Per-Agent Receipt Breakdown (DisclosureGroup)

    private var perStageReceiptBreakdown: some View {
        GroupBox {
            let sortedStages = stageSnapshots

            if sortedStages.isEmpty {
                Text("No stage data available.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(sortedStages) { stage in
                    DisclosureGroup {
                        VStack(alignment: .leading, spacing: 6) {
                            // Stage-level info
                            HStack(spacing: 12) {
                                LabeledContent("Status") {
                                    Text(stage.status.rawValue)
                                        .font(.caption.monospaced())
                                }
                                LabeledContent("Attempt") {
                                    Text("#\(stage.attemptNumber)")
                                        .font(.caption.monospaced())
                                }
                                if let completed = stage.completedAt {
                                    LabeledContent("Duration") {
                                        Text(formatDuration(from: stage.startedAt, to: completed))
                                            .font(.caption.monospaced())
                                    }
                                }
                            }
                            .font(.caption)

                            // Per-agent breakdown
                            ForEach(stage.agentExecutions) { agent in
                                VStack(alignment: .leading, spacing: 3) {
                                    HStack {
                                        Image(systemName: "person.circle")
                                            .font(.caption2)
                                            .foregroundStyle(.blue)
                                        Text(agent.agentTitle)
                                            .font(.caption.bold())
                                        Text("(\(agent.taskName))")
                                            .font(.caption2)
                                            .foregroundStyle(.secondary)
                                        Spacer()
                                        if let cost = agent.costCents {
                                            Text("\(cost)c")
                                                .font(.caption2.monospaced())
                                                .foregroundStyle(.green)
                                        }
                                        Text(agent.status.rawValue)
                                            .font(.caption2)
                                            .padding(.horizontal, 5)
                                            .padding(.vertical, 1)
                                            .background(agentStatusColor(agent.status).opacity(0.15))
                                            .foregroundStyle(agentStatusColor(agent.status))
                                            .clipShape(Capsule())
                                    }

                                    // Provider receipt indicator
                                    HStack(spacing: 8) {
                                        Text("\(agent.provider) / \(agent.resolvedModel ?? "default")")
                                            .font(.caption2.monospaced())
                                            .foregroundStyle(.secondary)
                                        if agent.providerReceiptPresent {
                                            Image(systemName: "checkmark.circle.fill")
                                                .font(.caption2)
                                                .foregroundStyle(.green)
                                                .help("Provider receipt captured")
                                        }
                                    }

                                    // Agent artifacts
                                    let agentArtifacts = allArtifacts.filter {
                                        $0.agentID == agent.agentID && $0.stageID == stage.stageID
                                    }
                                    if !agentArtifacts.isEmpty {
                                        ForEach(agentArtifacts) { artifact in
                                            HStack(spacing: 4) {
                                                Image(systemName: artifactIcon(artifact))
                                                    .font(.caption2)
                                                    .foregroundStyle(artifactColor(artifact))
                                                Text(artifact.name)
                                                    .font(.caption2.monospaced())
                                                    .lineLimit(1)
                                                if artifact.isPinned {
                                                    Image(systemName: "pin.fill")
                                                        .font(.caption2)
                                                        .foregroundStyle(.orange)
                                                }
                                            }
                                            .padding(.leading, 16)
                                        }
                                    }
                                }
                                .padding(.vertical, 2)
                                .padding(.leading, 8)
                            }
                        }
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: stageIcon(stage.status))
                                .foregroundStyle(stageColor(stage.status))
                            Text(stage.label)
                                .font(.subheadline.bold())
                            Spacer()
                            Text("\(stage.agentExecutions.count) agents")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text("\(stageArtifactCount(stage)) artifacts")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(.vertical, 2)
                }
            }
        } label: {
            Label("Per-Stage Breakdown", systemImage: "rectangle.stack")
        }
    }

    // MARK: - Export Actions (§7.3 — Footer)

    private var exportActionsSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                // Export evidence pack
                HStack(spacing: 12) {
                    Image(systemName: "shippingbox.fill")
                        .font(.title3)
                        .foregroundStyle(.blue)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Export Evidence Pack")
                            .font(.subheadline.bold())
                        Text("Run metadata, all artifacts, stage summary, agent detail, and screenshot checklist.")
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
                    .disabled(isExporting || run.deliveryConfigurationJSON == nil)
                    .accessibilityIdentifier("completed-run-export-evidence-pack")
                }

                Divider()

                // Export sign-off packet
                HStack(spacing: 12) {
                    Image(systemName: "checkmark.seal.fill")
                        .font(.title3)
                        .foregroundStyle(signOffSnapshot != nil ? .green : .secondary)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Export Sign-Off Packet")
                            .font(.subheadline.bold())
                        Text(signOffSnapshot != nil
                             ? "Full decision snapshot with median calculations and gate evaluation."
                             : "Not available — no sign-off decision associated with this run.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button {
                        exportSignOffPacket()
                    } label: {
                        Label("Export", systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(.bordered)
                    .disabled(isExporting || signOffSnapshot == nil)
                }

                Divider()

                // Reveal artifact root
                HStack(spacing: 12) {
                    Image(systemName: "folder.fill")
                        .font(.title3)
                        .foregroundStyle(.orange)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Reveal Artifact Root")
                            .font(.subheadline.bold())
                        Text(run.artifactRoot.isEmpty ? "No artifact root set" : run.artifactRoot)
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Spacer()
                    Button {
                        if !run.artifactRoot.isEmpty {
                            NSWorkspace.shared.activateFileViewerSelecting(
                                [URL(fileURLWithPath: run.artifactRoot)]
                            )
                        }
                    } label: {
                        Label("Reveal", systemImage: "folder")
                    }
                    .buttonStyle(.bordered)
                    .disabled(run.artifactRoot.isEmpty)
                }
            }
        } label: {
            Label("Export Actions", systemImage: "square.and.arrow.up")
        }
    }

    // MARK: - Data Loading

    private func loadRunData() {
        dataLoadWarning = nil

        // Load all artifacts for this run
        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID
            },
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        do {
            allArtifacts = try modelContext.fetch(descriptor)
        } catch {
            let message = "Failed to load run artifacts: \(error.localizedDescription)"
            dataLoadWarning = message
            ForgeLogger.ui.error("CompletedRunExportHub loadRunData failed for run \(run.id): \(error.localizedDescription)")
            allArtifacts = []
        }

        // Determine evidence pack status
        classifyEvidencePackStatus()

        // Look up sign-off snapshot for benchmark cohort
        loadSignOffSnapshot()
    }

    /// Proposal 008 (REQ-015): Classify evidence-pack status using benchmark truth when available.
    private func classifyEvidencePackStatus() {
        let hasDeliveryConfig = run.deliveryConfigurationJSON != nil
        let hasReceipts = allArtifacts.contains {
            $0.name.localizedCaseInsensitiveContains("receipt")
        }

        // Check for benchmark-side truth first (Proposal 008 REQ-015, REQ-020).
        // Derive .exported strictly from persisted evidencePackExportedAt for benchmark-linked
        // runs, not from receipt-presence heuristics. This keeps the operator-facing status
        // in sync with the evaluator's Gate 6 truth.
        if let cohortID = run.experimentCohortID {
            let pairDescriptor = FetchDescriptor<BenchmarkPair>()
            do {
                let allPairs = try modelContext.fetch(pairDescriptor)
                if let pair = allPairs.first(where: { $0.appDrivenRecord?.linkedRunID == run.id && $0.cohort?.id == cohortID }) {
                // Run is linked to a benchmark pair — derive status from persisted export truth.
                    if let appRecord = pair.appDrivenRecord, appRecord.evidencePackExportedAt != nil {
                        evidencePackStatus = .exported
                        return
                    } else if pair.appDrivenRecord != nil {
                        evidencePackStatus = .ready
                        return
                    }
                }
            } catch {
                let message = "Failed to load benchmark export truth: \(error.localizedDescription)"
                dataLoadWarning = dataLoadWarning ?? message
                ForgeLogger.ui.error("CompletedRunExportHub classifyEvidencePackStatus failed for run \(run.id): \(error.localizedDescription)")
            }
        }

        // Fallback to heuristic classification.
        if !hasDeliveryConfig {
            evidencePackStatus = .missing
        } else if run.status == .completed && hasReceipts {
            evidencePackStatus = .ready
        } else if run.status == .running || run.status == .ready {
            evidencePackStatus = .inProgress
        } else {
            evidencePackStatus = .missing
        }
    }

    private func loadSignOffSnapshot() {
        guard let cohortID = run.experimentCohortID else { return }
        let descriptor = FetchDescriptor<MVPSignOffDecisionSnapshot>(
            predicate: #Predicate<MVPSignOffDecisionSnapshot> { snapshot in
                snapshot.cohortID == cohortID
            },
            sortBy: [SortDescriptor(\.evaluatedAt, order: .reverse)]
        )
        do {
            signOffSnapshot = try modelContext.fetch(descriptor).first
        } catch {
            let message = "Failed to load sign-off snapshot: \(error.localizedDescription)"
            dataLoadWarning = dataLoadWarning ?? message
            ForgeLogger.ui.error("CompletedRunExportHub loadSignOffSnapshot failed for run \(run.id): \(error.localizedDescription)")
            signOffSnapshot = nil
        }
    }

    private func preferredExportDirectory() -> URL {
        let environment = ProcessInfo.processInfo.environment
        if let overridePath = environment["CHAINWORKS_UI_TEST_EXPORT_BASE_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           overridePath.isEmpty == false {
            let overrideURL = URL(fileURLWithPath: overridePath, isDirectory: true)
            try? FileManager.default.createDirectory(at: overrideURL, withIntermediateDirectories: true)
            return overrideURL
        }

        return FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
    }

    // MARK: - Export Operations

    private func exportEvidencePack() {
        isExporting = true
        exportMessage = nil

        let exportDirectory = preferredExportDirectory()
        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: FileManager.default.temporaryDirectory
                .appendingPathComponent("evidence-export-\(run.id.uuidString.prefix(8))", isDirectory: true),
            artifactRoot: FileManager.default.temporaryDirectory
                .appendingPathComponent("evidence-export-\(run.id.uuidString.prefix(8))/artifacts", isDirectory: true),
            worktreeRoot: run.worktreeRoot.map { URL(fileURLWithPath: $0) }
        )

        do {
            let pack = try EvidencePackBuilder.export(
                run: run,
                workspace: workspace,
                exportDirectory: exportDirectory
            )
            exportMessage = "Exported \(pack.itemCount) items to \(exportDirectory.lastPathComponent)."
            evidencePackStatus = .exported
            // Proposal 008 (REQ-020): Mark the linked benchmark record as exported
            // so the GO/HOLD gate can verify complete exported review packets.
            if let warning = markBenchmarkRecordExported() {
                exportMessage = "\(exportMessage ?? "Export completed.") Warning: \(warning)"
            }
        } catch {
            exportMessage = "Export failed: \(error.localizedDescription)"
        }

        isExporting = false
    }

    /// Proposal 008 (REQ-020): Stamp the benchmark execution record with the export timestamp
    /// so the evaluator's Gate 6 can verify complete exported review packets.
    private func markBenchmarkRecordExported() -> String? {
        do {
            _ = try BenchmarkExportStamping.markRunEvidencePackExported(
                runID: run.id,
                cohortID: run.experimentCohortID,
                context: modelContext
            )
            return nil
        } catch {
            ForgeLogger.ui.error("Evidence export stamp failed for run \(run.id): \(error.localizedDescription)")
            return "Benchmark export stamp failed: \(error.localizedDescription)"
        }
    }

    private func exportSignOffPacket() {
        guard let snapshot = signOffSnapshot else { return }
        isExporting = true
        exportMessage = nil

        let exportDirectory = preferredExportDirectory()

        do {
            let packetDir = exportDirectory
                .appendingPathComponent("signoff-packet-\(snapshot.id.uuidString.prefix(8))", isDirectory: true)
            try FileManager.default.createDirectory(at: packetDir, withIntermediateDirectories: true)

            // Decision metadata
            let metadata: [String: Any] = [
                "snapshotID": snapshot.id.uuidString,
                "cohortID": snapshot.cohortID.uuidString,
                "evaluatorVersion": snapshot.evaluatorVersion,
                "evaluatedAt": ISO8601DateFormatter().string(from: snapshot.evaluatedAt),
                "decision": snapshot.decision.rawValue,
                "payloadChecksum": snapshot.payloadChecksum,
                "pairCount": snapshot.pairCount,
                "happyPathCount": snapshot.happyPathCount,
                "recoveredCount": snapshot.recoveredCount,
                "medianImprovementPercent": snapshot.medianImprovementPercent ?? 0,
                "failingGateReasons": snapshot.failingGateReasons
            ] as [String: Any]

            let metaData = try JSONSerialization.data(
                withJSONObject: metadata,
                options: [.prettyPrinted, .sortedKeys]
            )
            try metaData.write(to: packetDir.appendingPathComponent("signoff-decision.json"))

            // Full decision payload
            try snapshot.decisionPayloadJSON.write(
                to: packetDir.appendingPathComponent("decision-payload.json")
            )

            exportMessage = "Sign-off packet exported to Desktop."
        } catch {
            exportMessage = "Sign-off export failed: \(error.localizedDescription)"
        }

        isExporting = false
    }

    private var releaseManifestArtifact: Artifact? {
        artifact(named: "release_manifest")
    }

    private var gitPushReceiptArtifact: Artifact? {
        artifact(named: "git_push_receipt")
    }

    private var uploadReceiptArtifact: Artifact? {
        artifact(named: "connect_upload_receipt")
            ?? artifact(named: "delivery_receipt")
    }

    // MARK: - Computed Properties

    private var resultIcon: String {
        switch run.status {
        case .completed: return "checkmark.circle.fill"
        case .failed: return "xmark.circle.fill"
        case .cancelled: return "stop.circle.fill"
        default: return "circle.fill"
        }
    }

    private var resultColor: Color {
        switch run.status {
        case .completed: return .green
        case .failed: return .red
        case .cancelled: return .gray
        default: return .secondary
        }
    }

    private var resultBadge: some View {
        Text(run.presentationStatusLabel.uppercased())
            .font(.caption.bold())
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(resultColor.opacity(0.15))
            .foregroundStyle(resultColor)
            .clipShape(Capsule())
    }

    private var formattedElapsed: String {
        let elapsed = (run.completedAt ?? Date()).timeIntervalSince(run.startedAt)
        let mins = Int(elapsed) / 60
        let secs = Int(elapsed) % 60
        if mins >= 60 {
            let hrs = mins / 60
            let remainMins = mins % 60
            return "\(hrs)h \(remainMins)m"
        }
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }

    private var formattedCost: String {
        guard let cost = run.totalCostCents else { return "--" }
        if cost >= 100 {
            return String(format: "$%.2f", Double(cost) / 100.0)
        }
        return "\(cost)c"
    }

    private var completedStageCount: Int {
        stageSnapshots.filter { $0.status == .completed }.count
    }

    // MARK: - Helpers

    private func formatDuration(from start: Date, to end: Date) -> String {
        let elapsed = end.timeIntervalSince(start)
        let mins = Int(elapsed) / 60
        let secs = Int(elapsed) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }

    private func stageArtifactCount(_ stage: RunStageSnapshot) -> Int {
        allArtifacts.filter { $0.stageID == stage.stageID && $0.attemptNumber == stage.attemptNumber }.count
    }

    private func artifact(named name: String) -> Artifact? {
        allArtifacts.first { $0.name == name || $0.contractID == name }
    }

    private func artifactContinuitySubtitle(for artifact: Artifact) -> String {
        let fileName = URL(fileURLWithPath: artifact.filePath).lastPathComponent
        return "\(fileName) · \(artifact.stageID)"
    }

    private func revealWorktree() {
        guard let worktreeRoot = run.worktreeRoot, worktreeRoot.isEmpty == false else { return }
        NSWorkspace.shared.activateFileViewerSelecting(
            [URL(fileURLWithPath: worktreeRoot)]
        )
    }

    private func stageIcon(_ status: StageStatus) -> String {
        switch status {
        case .completed: return "checkmark.circle.fill"
        case .failed: return "xmark.circle.fill"
        case .running: return "play.circle.fill"
        case .waitingApproval: return "pause.circle.fill"
        case .blocked: return "exclamationmark.triangle.fill"
        case .skipped: return "forward.fill"
        case .pending, .ready: return "circle"
        }
    }

    private func stageColor(_ status: StageStatus) -> Color {
        switch status {
        case .completed: return .green
        case .failed: return .red
        case .running: return .blue
        case .waitingApproval: return .orange
        case .blocked: return .red
        case .skipped: return .gray
        case .pending, .ready: return .secondary
        }
    }

    private func agentStatusColor(_ status: AgentStatus) -> Color {
        switch status {
        case .completed: return .green
        case .failed: return .red
        case .running: return .blue
        case .cancelled: return .gray
        case .skipped: return .gray
        case .pending, .ready: return .secondary
        }
    }

    private func artifactIcon(_ artifact: Artifact) -> String {
        if artifact.name.localizedCaseInsensitiveContains("receipt") {
            return "doc.seal"
        }
        switch artifact.format {
        case .json: return "curlybraces"
        case .markdown: return "doc.richtext"
        case .diff: return "chevron.left.forwardslash.chevron.right"
        case .report: return "doc.text"
        }
    }

    private func artifactColor(_ artifact: Artifact) -> Color {
        if artifact.name.localizedCaseInsensitiveContains("receipt") {
            return .green
        }
        switch artifact.format {
        case .json: return .orange
        case .markdown: return .blue
        case .diff: return .purple
        case .report: return .secondary
        }
    }
}
