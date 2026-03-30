import SwiftUI
import SwiftData

// MARK: - P005-OPS §5: Runs Home View

/// Primary operator landing surface.
/// Answers: "What needs my attention right now, and what safe action is available?"
/// Runs grouped into: Waiting Approval, Blocked, Running, Recently Completed.
struct RunsHomeView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings

    @Query(sort: \Run.startedAt, order: .reverse)
    private var allRuns: [Run]

    @State private var selectedRun: Run?
    @State private var showRecoverySheet = false
    @State private var showComparisonPicker = false
    @State private var comparisonTargetRun: Run?
    @State private var showReportView = false
    // Proposal 008 (§7.1–7.2): Blocked run recovery deep-link
    @State private var showBlockedRecovery = false

    var body: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
            HStack(alignment: .firstTextBaseline, spacing: DesignTokens.Spacing.small) {
                Text("Runs Home")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(DesignTokens.Status.neutral)
                    .accessibilityIdentifier("runs-home-owner-ready")

                Spacer(minLength: DesignTokens.Spacing.small)

                Text(runsHomeAccessibilitySummary)
                    .font(.caption2)
                    .foregroundStyle(DesignTokens.Status.neutral)
                    .multilineTextAlignment(.trailing)
                    .accessibilityLabel(runsHomeAccessibilityLabel)
                    .accessibilityValue(runsHomeAccessibilityValue)
                    .accessibilityAddTraits(.isStaticText)
                    .accessibilityIdentifier("runs-home-adopter-slice-summary-text")
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(runsHomeAccessibilityLabel)
            .accessibilityValue(runsHomeAccessibilityValue)
            .accessibilityIdentifier("runs-home-adopter-slice-summary")
            .padding(.horizontal, DesignTokens.Spacing.section)
            .padding(.top, DesignTokens.Spacing.compact)

            NavigationSplitView {
                List(selection: $selectedRun) {
                // §5.2: Waiting Approval
                if !waitingApprovalRuns.isEmpty {
                    Text("Waiting Approval")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(DesignTokens.Status.warning)
                        .accessibilityIdentifier("runs-home-section-waiting-approval")

                    Section {
                        ForEach(waitingApprovalRuns) { run in
                            RunsHomeRow(
                                run: run,
                                attentionLevel: .high,
                                onOpen: { selectedRun = run },
                                onOpenGate: { resolveApprovalGate(for: run) },
                                onRecover: { selectedRun = run; showRecoverySheet = true },
                                onCompare: { selectedRun = run; showComparisonPicker = true },
                                onViewReport: { selectedRun = run; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(run)
                        }
                    } header: {
                        Label("Waiting Approval", systemImage: "checkmark.seal")
                            .foregroundStyle(DesignTokens.Status.warning)
                    }
                }

                // §5.2: Blocked
                if !blockedRuns.isEmpty {
                    Section {
                        ForEach(blockedRuns) { run in
                            RunsHomeRow(
                                run: run,
                                attentionLevel: .critical,
                                onOpen: { selectedRun = run },
                                onOpenGate: nil,
                                onRecover: { selectedRun = run; showRecoverySheet = true },
                                onCompare: { selectedRun = run; showComparisonPicker = true },
                                onViewReport: { selectedRun = run; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(run)
                        }
                    } header: {
                        Label("Blocked", systemImage: "exclamationmark.triangle")
                            .foregroundStyle(DesignTokens.Status.error)
                    }
                }

                // §5.2: Running
                if !runningRuns.isEmpty {
                    Section {
                        ForEach(runningRuns) { run in
                            RunsHomeRow(
                                run: run,
                                attentionLevel: .normal,
                                onOpen: { selectedRun = run },
                                onOpenGate: nil,
                                onRecover: nil,
                                onCompare: { selectedRun = run; showComparisonPicker = true },
                                onViewReport: { selectedRun = run; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(run)
                        }
                    } header: {
                        Label("Running", systemImage: "play.fill")
                            .foregroundStyle(DesignTokens.Status.running)
                    }
                }

                // §5.2: Recently Completed
                if !recentlyCompletedRuns.isEmpty {
                    Section {
                        ForEach(recentlyCompletedRuns) { run in
                            RunsHomeRow(
                                run: run,
                                attentionLevel: .low,
                                onOpen: { selectedRun = run },
                                onOpenGate: nil,
                                onRecover: (run.status == .failed) ? { selectedRun = run; showRecoverySheet = true } : nil,
                                onCompare: { selectedRun = run; showComparisonPicker = true },
                                onViewReport: { selectedRun = run; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(run)
                        }
                    } header: {
                        Label("Recently Completed", systemImage: "checkmark.circle")
                            .foregroundStyle(DesignTokens.Status.success)
                    }
                }

                if allRuns.isEmpty {
                    // Proposal 012 (L-01): Enhanced empty state
                    StyledEmptyState(
                        title: "No Runs",
                        systemImage: "tray",
                        description: "Start a run from the Ideas tab to see it here."
                    )
                }
            }
                .navigationTitle("Runs Home")
                .accessibilityIdentifier("runs-home-list")
                // Proposal 012 (C-01): Widen sidebar to accommodate run row content
                .navigationSplitViewColumnWidth(min: 280, ideal: 340)
            } detail: {
                if let run = selectedRun {
                    RunDetailPanel(
                        run: run,
                        onRecover: { showRecoverySheet = true },
                        onCompare: { showComparisonPicker = true },
                        onViewReport: { showReportView = true },
                        onBlockedRecovery: { showBlockedRecovery = true },
                        compatibilityChecker: compatibilityChecker
                    )
                } else {
                    // Proposal 012 (L-01): Enhanced empty state
                    StyledEmptyState(
                        title: "Select a Run",
                        systemImage: "sidebar.left",
                        description: "Choose a run from the sidebar to view details."
                    )
                }
            }
        }
        .sheet(isPresented: $showRecoverySheet) {
            if let run = selectedRun {
                RecoverySheet(run: run)
            }
        }
        .sheet(isPresented: $showComparisonPicker) {
            if let run = selectedRun {
                RunComparisonPickerSheet(run: run, compatibilityChecker: compatibilityChecker) { targetRun in
                    comparisonTargetRun = targetRun
                    showComparisonPicker = false
                }
            }
        }
        .sheet(item: $comparisonTargetRun) { targetRun in
            if let run = selectedRun {
                RunComparisonView(runA: run, runB: targetRun)
            }
        }
        .sheet(isPresented: $showReportView) {
            if let run = selectedRun {
                NavigationStack {
                    RunReportView(run: run)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Done") { showReportView = false }
                            }
                        }
                }
                .frame(minWidth: 600, minHeight: 500)
            }
        }
        // Proposal 008 (§7.2): Blocked run recovery deep-link surface
        .sheet(isPresented: $showBlockedRecovery) {
            if let run = selectedRun {
                NavigationStack {
                    BlockedRunRecoveryView(run: run)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Done") { showBlockedRecovery = false }
                            }
                        }
                }
                .frame(minWidth: 600, minHeight: 500)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(runsHomeAccessibilityLabel)
        .accessibilityValue(runsHomeAccessibilityValue)
        .accessibilityIdentifier("runs-home-owner-view")
        .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenRunInRunsHome)) { notification in
            guard
                let runIDString = notification.userInfo?["runID"] as? String,
                let runID = UUID(uuidString: runIDString)
            else { return }
            if let run = allRuns.first(where: { $0.id == runID }) {
                selectedRun = run
            }
        }
    }

    // MARK: - Grouped Runs (§5.2)

    private var waitingApprovalRuns: [Run] {
        allRuns.filter { $0.status == .waitingApproval }
    }

    private var blockedRuns: [Run] {
        allRuns.filter { $0.status == .blocked }
    }

    private var runningRuns: [Run] {
        allRuns.filter { $0.status == .running || $0.status == .ready || $0.status == .pending }
    }

    private var recentlyCompletedRuns: [Run] {
        allRuns.filter { $0.status == .completed || $0.status == .failed || $0.status == .cancelled }
    }

    private var runsHomeAccessibilitySummary: String {
        "Waiting approval \(waitingApprovalRuns.count), blocked \(blockedRuns.count), recent completed \(recentlyCompletedRuns.count)"
    }

    private var runsHomeAccessibilityLabel: String {
        "Runs Home. \(runsHomeAccessibilitySummary). Accessibility display settings: \(runsHomeAccessibilityValue)"
    }

    private var runsHomeAccessibilityValue: String {
        var modes: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            modes.append("differentiate without color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            modes.append("increase contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            modes.append("reduce transparency")
        }
        let modeSummary = modes.isEmpty ? "standard accessibility display settings" : modes.joined(separator: ", ")
        if let featuredRun = waitingApprovalRuns.first ?? blockedRuns.first ?? runningRuns.first ?? recentlyCompletedRuns.first {
            let featuredTitle = featuredRun.idea?.title ?? "Unknown Idea"
            return "\(modeSummary). Featured run: \(featuredTitle), \(featuredRun.presentationStatusLabel)."
        }
        return modeSummary
    }

    // MARK: - Compatibility (§8.1)

    /// Uses RunComparisonService to check true compatibility rather than "any sibling run".
    private var compatibilityChecker: CompatibilityChecker {
        CompatibilityChecker(modelContext: modelContext)
    }

    // MARK: - Approval Gate Resolution

    private func resolveApprovalGate(for run: Run) {
        // Find the pending approval for this run and resolve it via ExecutionService
        if let approvalEntry = executionService.pendingApprovals.first(where: { $0.value.runID == run.id }) {
            executionService.resolveApproval(approvalID: approvalEntry.key, granted: true)
        }
    }
}

// MARK: - Compatibility Checker

/// Wraps RunComparisonService for use in views. Determines true compatibility.
struct CompatibilityChecker {
    let modelContext: ModelContext

    func hasCompatibleTargets(for run: Run) -> Bool {
        let service = RunComparisonService(modelContext: modelContext)
        return !service.compatibleTargets(for: run).isEmpty
    }

    func compatibleTargets(for run: Run) -> [Run] {
        let service = RunComparisonService(modelContext: modelContext)
        return service.compatibleTargets(for: run)
    }
}

// MARK: - Run Row (§5.3)

struct RunsHomeRow: View {
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings

    let run: Run
    let attentionLevel: AttentionLevel

    // §5.4: Real executable action callbacks — only non-nil actions are shown
    let onOpen: () -> Void
    let onOpenGate: (() -> Void)?
    let onRecover: (() -> Void)?
    let onCompare: (() -> Void)?
    let onViewReport: (() -> Void)?
    let compatibilityChecker: CompatibilityChecker

    enum AttentionLevel {
        case critical, high, normal, low

        var color: Color {
            switch self {
            case .critical: return .red
            case .high: return .orange
            case .normal: return .blue
            case .low: return .secondary
            }
        }

        var icon: String {
            switch self {
            case .critical: return "exclamationmark.triangle.fill"
            case .high: return "bell.badge.fill"
            case .normal: return "play.circle.fill"
            case .low: return "checkmark.circle"
            }
        }
    }

    var body: some View {
        // Proposal 012 (C-01 / H-04): Compact 2-line sidebar row.
        // Full details (parent idea badge, provenance, cost, last progress) live in RunDetailPanel.
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
            // Line 1: Title + attention icon
            HStack {
                Text(run.idea?.title ?? "Unknown Idea")
                    .font(DesignTokens.Typography.cardTitle)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer()
                Image(systemName: attentionLevel.icon)
                    .font(DesignTokens.Typography.supporting)
                    .foregroundStyle(attentionLevel.color)
            }

            // Line 2: Status capsule + elapsed time
            HStack(spacing: DesignTokens.Spacing.small) {
                StatusCapsule(
                    text: run.presentationStatusLabel,
                    color: statusColor,
                    size: .small,
                    accessibilityIdentifier: "run-row-status-\(sanitizedRunTitle)"
                )
                if let stageLabel = currentStageLabel {
                    Text(stageLabel)
                        .font(DesignTokens.Typography.micro)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer()
                Text(elapsedTimeString)
                    .font(DesignTokens.Typography.micro)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, DesignTokens.Spacing.compact)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(rowAccessibilityLabel)
        .accessibilityValue(rowStatusAccessibilityValue)
        .accessibilityIdentifier("run-row-title-\(sanitizedRunTitle)")
        // §5.4: Contextual row actions — only executable actions appear
        .contextMenu {
            Button("Open", systemImage: "arrow.right.circle") { onOpen() }

            if let onOpenGate, run.status == .waitingApproval {
                Button("Open Gate", systemImage: "checkmark.seal") { onOpenGate() }
            }

            if let onRecover, run.status == .blocked || run.status == .failed {
                Button("Recover", systemImage: "arrow.counterclockwise") { onRecover() }
            }

            if let onCompare, compatibilityChecker.hasCompatibleTargets(for: run) {
                Button("Compare", systemImage: "arrow.left.arrow.right") { onCompare() }
            }

            if let onViewReport, run.latestImmutableReportArtifactID != nil {
                Button("View Report", systemImage: "doc.text") { onViewReport() }
            }
        }
    }

    // MARK: - Computed

    // Proposal 012 (M-02): Semantic status colors
    private var statusColor: Color {
        switch run.presentationStatus {
        case .completed: return DesignTokens.Status.success
        case .failed: return DesignTokens.Status.error
        case .blocked: return DesignTokens.Status.error
        case .waitingApproval: return DesignTokens.Status.warning
        case .running: return DesignTokens.Status.running
        case .cancelled: return DesignTokens.Status.cancelled
        case .cancelling: return DesignTokens.Status.warning
        case .pending, .ready: return DesignTokens.Status.neutral
        }
    }

    private var currentStageLabel: String? {
        guard let stageID = run.currentStageID else { return nil }
        return run.stageExecutions.first(where: { $0.stageID == stageID })?.label
    }

    private var sanitizedRunTitle: String {
        (run.idea?.title ?? "unknown-idea")
            .lowercased()
            .replacingOccurrences(of: " ", with: "-")
    }

    private var rowStatusAccessibilityValue: String {
        var modes: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            modes.append("differentiate without color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            modes.append("increase contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            modes.append("reduce transparency")
        }
        return modes.isEmpty ? "standard accessibility display settings" : modes.joined(separator: ", ")
    }

    private var rowAccessibilityLabel: String {
        var parts = [run.idea?.title ?? "Unknown Idea", run.presentationStatusLabel]
        if let currentStageLabel {
            parts.append(currentStageLabel)
        }
        parts.append(elapsedTimeString)
        return parts.joined(separator: ", ")
    }

    private var elapsedTimeString: String {
        let elapsed = (run.completedAt ?? Date()).timeIntervalSince(run.startedAt)
        let mins = Int(elapsed) / 60
        let secs = Int(elapsed) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }

    private var lastProgressString: String {
        let sorted = run.stageExecutions.sorted { $0.startedAt < $1.startedAt }
        let lastDate = sorted.last?.completedAt ?? sorted.last?.startedAt ?? run.startedAt
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: lastDate, relativeTo: Date())
    }
}

#Preview("Runs Home — Mixed States") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

    return RunsHomeView()
        .modelContainer(container)
        .environment(executionService)
        .frame(width: 1280, height: 820)
}

// MARK: - Runtime Provenance Badge (§5.3)

// Proposal 012 (M-01): Migrated to StatusCapsule pattern with DesignTokens
struct RuntimeProvenanceBadge: View {
    let trustLevel: String?

    var body: some View {
        StatusCapsule(
            text: badgeLabel,
            color: badgeColor,
            icon: badgeIcon,
            size: .small
        )
    }

    private var badgeLabel: String {
        switch trustLevel {
        case "fixture_verified": return "Fixture / verified"
        case "server_unverified": return "Goose server / trust pending"
        case "server_verified": return "Goose server / verified"
        default: return "Unknown"
        }
    }

    private var badgeIcon: String {
        switch trustLevel {
        case "fixture_verified": return "checkmark.shield.fill"
        case "server_verified": return "checkmark.shield.fill"
        case "server_unverified": return "shield.lefthalf.filled"
        default: return "questionmark.circle"
        }
    }

    private var badgeColor: Color {
        switch trustLevel {
        case "fixture_verified": return DesignTokens.Status.success
        case "server_verified": return DesignTokens.Status.success
        case "server_unverified": return DesignTokens.Status.warning
        default: return DesignTokens.Status.neutral
        }
    }
}

// MARK: - Parent Idea Archive Badge

/// Read-only truth surface for the parent idea lifecycle on run-centric screens.
/// Does not expose any restore/modify action.
struct ParentIdeaArchiveBadge: View {
    let title: String
    let idea: Idea?

    var body: some View {
        StatusCapsule(
            text: "\(title): \(statusText)",
            color: statusColor,
            icon: statusIcon,
            size: .small,
            accessibilityIdentifier: "parent-idea-archive-\(sanitizedTitle)"
        )
    }

    private var sanitizedTitle: String {
        title.lowercased().replacingOccurrences(of: " ", with: "-")
    }

    private var statusText: String {
        guard let idea else { return "Unavailable" }
        return idea.lifecycleStatusLabel
    }

    private var statusIcon: String {
        guard let idea else { return "questionmark.circle" }
        if idea.isArchived { return "archivebox.fill" }
        if let latestRun = idea.latestRun {
            switch latestRun.presentationStatus {
            case .pending, .ready:
                return "clock.fill"
            case .running:
                return "play.circle.fill"
            case .waitingApproval:
                return "checkmark.seal.fill"
            case .blocked:
                return "pause.circle.fill"
            case .completed:
                return "checkmark.circle.fill"
            case .failed:
                return "xmark.circle.fill"
            case .cancelled:
                return "stop.circle.fill"
            case .cancelling:
                return "hourglass"
            }
        }
        return "lightbulb.fill"
    }

    private var statusColor: Color {
        guard let idea else { return .secondary }
        if idea.isArchived { return .secondary }
        if let latestRun = idea.latestRun {
            switch latestRun.presentationStatus {
            case .pending, .ready:
                return DesignTokens.Status.neutral
            case .running:
                return DesignTokens.Status.running
            case .waitingApproval, .cancelling:
                return DesignTokens.Status.warning
            case .blocked:
                return DesignTokens.Status.warning
            case .completed:
                return DesignTokens.Status.success
            case .failed:
                return DesignTokens.Status.error
            case .cancelled:
                return DesignTokens.Status.cancelled
            }
        }
        return DesignTokens.Status.success
    }
}

// MARK: - Run Detail Panel

struct RunDetailPanel: View {
    let run: Run
    let onRecover: () -> Void
    let onCompare: () -> Void
    let onViewReport: () -> Void
    // Proposal 008 (§7.2): Blocked run recovery deep-link
    var onBlockedRecovery: (() -> Void)?
    let compatibilityChecker: CompatibilityChecker

    @Environment(\.modelContext) private var modelContext
    @State private var evidenceExportMessage: String?

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
                    VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
                        Text(run.idea?.title ?? "Unknown Idea")
                            .font(.title2.bold())
                        Text(run.workflowTitle)
                            .font(.title3)
                            .foregroundStyle(.secondary)
                        HStack {
                            StatusCapsule(
                                text: run.presentationStatusLabel,
                                color: statusColor,
                                size: .regular
                            )
                            RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
                        }
                        ParentIdeaArchiveBadge(title: "Parent idea", idea: run.idea)
                    }

                    Divider()

                    LabeledContent("Started", value: run.startedAt.formatted())
                    if let completed = run.completedAt {
                        LabeledContent("Completed", value: completed.formatted())
                    }
                    LabeledContent("Elapsed", value: elapsedTimeString)
                    if let cost = run.totalCostCents {
                        LabeledContent("Total Cost", value: "\(cost) cents")
                    }

                    Divider()

                    Text("Stages")
                        .font(DesignTokens.Typography.sectionHeader)
                    ForEach(run.stageExecutions.sorted(by: { $0.startedAt < $1.startedAt })) { stage in
                        HStack {
                            Image(systemName: stageIcon(stage.status))
                                .foregroundStyle(stageColor(stage.status))
                            Text(stage.label)
                            Spacer()
                            Text(stage.status.rawValue)
                                .font(DesignTokens.Typography.supporting)
                                .foregroundStyle(.secondary)
                        }
                    }

                    Divider()

                    Text("Workflow Map")
                        .font(DesignTokens.Typography.sectionHeader)
                    WorkflowMapView(run: run)
                }
                .padding()
            }

            // Proposal 012 (L-10): Sticky action footer — always visible above the fold
            if hasAnyAction {
                Divider()
                VStack(spacing: DesignTokens.Spacing.small) {
                    HStack(spacing: DesignTokens.Spacing.medium) {
                        if run.status == .blocked || run.status == .failed {
                            Button("Recover", systemImage: "arrow.counterclockwise") {
                                onRecover()
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(DesignTokens.Action.caution)

                            // Proposal 008 (§7.2): Detailed blocked-run recovery surface
                            if let onBlockedRecovery {
                                Button("Detailed Recovery", systemImage: "wrench.and.screwdriver") {
                                    onBlockedRecovery()
                                }
                                .buttonStyle(.bordered)
                            }
                        }

                        if compatibilityChecker.hasCompatibleTargets(for: run) {
                            Button("Compare", systemImage: "arrow.left.arrow.right") {
                                onCompare()
                            }
                            .buttonStyle(.bordered)
                        }

                        if run.latestImmutableReportArtifactID != nil {
                            Button("View Report", systemImage: "doc.text") {
                                onViewReport()
                            }
                            .buttonStyle(.bordered)
                        }

                        // Gap 2 (Proposal 007): Export Evidence Pack for completed delivery runs
                        if (run.status == .completed || run.status == .failed),
                           run.deliveryConfigurationJSON != nil {
                            Button("Export Evidence Pack", systemImage: "shippingbox") {
                                exportEvidencePack()
                            }
                            .buttonStyle(.bordered)
                            .accessibilityIdentifier("export-evidence-pack-button")
                        }
                    }

                    if let evidenceExportMessage {
                        Text(evidenceExportMessage)
                            .font(DesignTokens.Typography.supporting)
                            .foregroundStyle(.secondary)
                            .transition(.opacity)
                    }
                }
                .padding()
                .background(.bar)
            }
        }
        .navigationTitle("Run Details")
        .accessibilityIdentifier("run-detail-panel")
    }

    private var hasAnyAction: Bool {
        run.status == .blocked || run.status == .failed
            || compatibilityChecker.hasCompatibleTargets(for: run)
            || run.latestImmutableReportArtifactID != nil
            || (run.deliveryConfigurationJSON != nil && (run.status == .completed || run.status == .failed))
    }

    private func exportEvidencePack() {
        let desktopURL = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: FileManager.default.temporaryDirectory
                .appendingPathComponent("evidence-export-\(run.id.uuidString.prefix(8))", isDirectory: true),
            artifactRoot: FileManager.default.temporaryDirectory
                .appendingPathComponent("evidence-export-\(run.id.uuidString.prefix(8))/artifacts", isDirectory: true),
            worktreeRoot: run.worktreeRoot.map { URL(fileURLWithPath: $0) }
        )
        do {
            let pack = try EvidencePackBuilder.export(run: run, workspace: workspace, exportDirectory: desktopURL)
            evidenceExportMessage = "Exported \(pack.itemCount) items to Desktop."
            // Proposal 008 (REQ-020): Mark linked benchmark record as exported.
            markBenchmarkRecordExported()
        } catch {
            evidenceExportMessage = "Export failed: \(error.localizedDescription)"
        }
    }

    /// Proposal 008 (REQ-020): Stamp the benchmark execution record with the export timestamp.
    private func markBenchmarkRecordExported() {
        guard let cohortID = run.experimentCohortID else { return }
        let pairDescriptor = FetchDescriptor<BenchmarkPair>()
        guard let allPairs = try? modelContext.fetch(pairDescriptor),
              let pair = allPairs.first(where: {
                  $0.appDrivenRecord?.linkedRunID == run.id && $0.cohort?.id == cohortID
              }),
              let appRecord = pair.appDrivenRecord else { return }
        appRecord.evidencePackExportedAt = Date()
        try? modelContext.save()
    }

    // Proposal 012 (M-02): Semantic status colors
    private var statusColor: Color {
        switch run.presentationStatus {
        case .completed: return DesignTokens.Status.success
        case .failed, .blocked: return DesignTokens.Status.error
        case .waitingApproval: return DesignTokens.Status.warning
        case .running: return DesignTokens.Status.running
        case .cancelling: return DesignTokens.Status.warning
        case .cancelled: return DesignTokens.Status.cancelled
        default: return DesignTokens.Status.neutral
        }
    }

    private var elapsedTimeString: String {
        let elapsed = (run.completedAt ?? Date()).timeIntervalSince(run.startedAt)
        let mins = Int(elapsed) / 60
        let secs = Int(elapsed) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
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
        case .completed: return DesignTokens.Status.success
        case .failed: return DesignTokens.Status.error
        case .running: return DesignTokens.Status.running
        case .waitingApproval: return DesignTokens.Status.warning
        case .blocked: return DesignTokens.Status.error
        case .skipped: return DesignTokens.Status.neutral
        case .pending, .ready: return DesignTokens.Status.neutral
        }
    }
}

// MARK: - Comparison Picker Sheet (§8.1 — true compatibility)

struct RunComparisonPickerSheet: View {
    let run: Run
    let compatibilityChecker: CompatibilityChecker
    let onSelect: (Run) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                let targets = compatibilityChecker.compatibleTargets(for: run)
                if targets.isEmpty {
                    ContentUnavailableView(
                        "No Compatible Runs",
                        systemImage: "arrow.left.arrow.right",
                        description: Text("No runs with the same idea and workflow family are available for comparison.")
                    )
                } else {
                    ForEach(targets.sorted(by: { $0.startedAt > $1.startedAt })) { target in
                        Button {
                            onSelect(target)
                        } label: {
                            VStack(alignment: .leading) {
                                Text(target.workflowTitle)
                                    .font(.headline)
                                HStack {
                                    Text(target.presentationStatusLabel)
                                        .font(.caption)
                                    Text("Started \(target.startedAt.formatted())")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                    Spacer()
                                    RuntimeProvenanceBadge(trustLevel: target.runtimeTrustLevel)
                                }
                            }
                        }
                    }
                }
            }
            .navigationTitle("Select Run to Compare")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}
