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

    @State private var selectedRunID: UUID?
    @State private var showRecoverySheet = false
    @State private var showComparisonPicker = false
    @State private var comparisonTargetRun: Run?
    @State private var showReportView = false
    // Proposal 008 (§7.1–7.2): Blocked run recovery deep-link
    @State private var showBlockedRecovery = false
    @State private var showCleanupConfirmation = false
    @State private var maintenanceInFlight = false
    @State private var maintenanceNotice: String?
    @State private var maintenanceErrorMessage: String?
    @State private var showMaintenanceError = false

    struct PresentationEntry: Identifiable {
        let run: Run
        let effectiveStatus: RunStatus

        var id: UUID { run.id }
    }

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

            if let maintenanceNotice {
                Text(maintenanceNotice)
                    .font(.caption)
                    .foregroundStyle(DesignTokens.Status.neutral)
                    .padding(.horizontal, DesignTokens.Spacing.section)
                    .accessibilityIdentifier("runs-home-maintenance-notice")
            }

            NavigationSplitView {
                List(selection: $selectedRunID) {
                // §5.2: Waiting Approval
                if !waitingApprovalRuns.isEmpty {
                    Text("Waiting Approval")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(DesignTokens.Status.warning)
                        .accessibilityIdentifier("runs-home-section-waiting-approval")

                    Section {
                        ForEach(waitingApprovalRuns) { entry in
                            RunsHomeRow(
                                run: entry.run,
                                effectiveStatus: entry.effectiveStatus,
                                attentionLevel: .high,
                                onOpen: { selectedRunID = entry.run.id },
                                onOpenGate: { resolveApprovalGate(for: entry.run) },
                                onRecover: { selectedRunID = entry.run.id; showRecoverySheet = true },
                                onCompare: { selectedRunID = entry.run.id; showComparisonPicker = true },
                                onViewReport: { selectedRunID = entry.run.id; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(Optional.some(entry.run.id))
                        }
                    } header: {
                        Label("Waiting Approval", systemImage: "checkmark.seal")
                            .foregroundStyle(DesignTokens.Status.warning)
                    }
                }

                // §5.2: Blocked
                if !blockedRuns.isEmpty {
                    Section {
                        ForEach(blockedRuns) { entry in
                            RunsHomeRow(
                                run: entry.run,
                                effectiveStatus: entry.effectiveStatus,
                                attentionLevel: .critical,
                                onOpen: { selectedRunID = entry.run.id },
                                onOpenGate: nil,
                                onRecover: { selectedRunID = entry.run.id; showRecoverySheet = true },
                                onCompare: { selectedRunID = entry.run.id; showComparisonPicker = true },
                                onViewReport: { selectedRunID = entry.run.id; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(Optional.some(entry.run.id))
                        }
                    } header: {
                        Label("Blocked", systemImage: "exclamationmark.triangle")
                            .foregroundStyle(DesignTokens.Status.error)
                    }
                }

                // §5.2: Running
                if !runningRuns.isEmpty {
                    Section {
                        ForEach(runningRuns) { entry in
                            RunsHomeRow(
                                run: entry.run,
                                effectiveStatus: entry.effectiveStatus,
                                attentionLevel: .normal,
                                onOpen: { selectedRunID = entry.run.id },
                                onOpenGate: nil,
                                onRecover: nil,
                                onCompare: { selectedRunID = entry.run.id; showComparisonPicker = true },
                                onViewReport: { selectedRunID = entry.run.id; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(Optional.some(entry.run.id))
                        }
                    } header: {
                        Label("Running", systemImage: "play.fill")
                            .foregroundStyle(DesignTokens.Status.running)
                    }
                }

                // §5.2: Recently Completed
                if !recentlyCompletedRuns.isEmpty {
                    Section {
                        ForEach(recentlyCompletedRuns) { entry in
                            RunsHomeRow(
                                run: entry.run,
                                effectiveStatus: entry.effectiveStatus,
                                attentionLevel: .low,
                                onOpen: { selectedRunID = entry.run.id },
                                onOpenGate: nil,
                                onRecover: (entry.effectiveStatus == .failed) ? { selectedRunID = entry.run.id; showRecoverySheet = true } : nil,
                                onCompare: { selectedRunID = entry.run.id; showComparisonPicker = true },
                                onViewReport: { selectedRunID = entry.run.id; showReportView = true },
                                compatibilityChecker: compatibilityChecker
                            )
                            .tag(Optional.some(entry.run.id))
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
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                if maintenanceInFlight {
                    ProgressView()
                        .controlSize(.small)
                        .accessibilityIdentifier("runs-home-maintenance-progress")
                }

                if interruptedRunCount > 0 {
                    Button("Resume Interrupted") {
                        resumeInterruptedRunsManually()
                    }
                    .disabled(maintenanceInFlight)
                    .accessibilityIdentifier("runs-home-resume-interrupted")
                }

                if cleanupCandidateCount > 0 {
                    Button("Clear Old Runs", role: .destructive) {
                        showCleanupConfirmation = true
                    }
                    .disabled(maintenanceInFlight)
                    .accessibilityIdentifier("runs-home-clear-old-runs")
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
        .confirmationDialog(
            "Clear Old Runs?",
            isPresented: $showCleanupConfirmation,
            titleVisibility: .visible
        ) {
            Button("Clear \(cleanupCandidateCount) Terminal Runs", role: .destructive) {
                clearOldRuns()
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("This removes completed, failed, and cancelled runs plus their owned run directories. Active, blocked, and waiting approval runs stay intact.")
        }
        .alert("Runs Maintenance Failed", isPresented: $showMaintenanceError) {
            Button("OK", role: .cancel) { }
        } message: {
            Text(maintenanceErrorMessage ?? "Unknown maintenance error.")
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
            if allRuns.contains(where: { $0.id == runID }) {
                selectedRunID = runID
            }
        }
    }

    // MARK: - Grouped Runs (§5.2)

    private var projectionService: WorkflowMapProjectionService {
        WorkflowMapProjectionService(modelContext: modelContext, executionService: executionService)
    }

    private var presentationEntries: [PresentationEntry] {
        let service = projectionService
        return buildRunsHomePresentationEntries(for: allRuns) { run in
            service.runStatus(for: run)
        }
    }

    private var waitingApprovalRuns: [PresentationEntry] {
        presentationEntries.filter { $0.effectiveStatus == .waitingApproval }
    }

    private var blockedRuns: [PresentationEntry] {
        presentationEntries.filter { $0.effectiveStatus == .blocked }
    }

    private var runningRuns: [PresentationEntry] {
        presentationEntries.filter {
            $0.effectiveStatus == .running
                || $0.effectiveStatus == .ready
                || $0.effectiveStatus == .pending
        }
    }

    private var recentlyCompletedRuns: [PresentationEntry] {
        presentationEntries.filter {
            $0.effectiveStatus == .completed
                || $0.effectiveStatus == .failed
                || $0.effectiveStatus == .cancelled
        }
    }

    private var runsHomeAccessibilitySummary: String {
        "Waiting approval \(waitingApprovalRuns.count), blocked \(blockedRuns.count), recent completed \(recentlyCompletedRuns.count)"
    }

    private var selectedRun: Run? {
        guard let selectedRunID else { return nil }
        return allRuns.first(where: { $0.id == selectedRunID })
    }

    private var interruptedRunCount: Int {
        allRuns.reduce(into: 0) { count, run in
            if run.status == .running || run.status == .waitingApproval {
                count += 1
            }
        }
    }

    private var cleanupCandidateCount: Int {
        allRuns.reduce(into: 0) { count, run in
            switch run.status {
            case .completed, .failed, .cancelled:
                count += 1
            default:
                break
            }
        }
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
            let featuredTitle = featuredRun.run.idea?.title ?? "Unknown Idea"
            let featuredStatus = featuredRun.effectiveStatus.rawValue.replacingOccurrences(of: "_", with: " ")
            return "\(modeSummary). Featured run: \(featuredTitle), \(featuredStatus)."
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

    private func resumeInterruptedRunsManually() {
        maintenanceInFlight = true
        maintenanceNotice = "Resuming interrupted runs manually."

        Task { @MainActor in
            let compiler = RunPlanCompiler(modelContext: modelContext)
            executionService.resumeInterruptedRuns(compiler: compiler)
            maintenanceInFlight = false
            maintenanceNotice = interruptedRunCount == 0
                ? "Interrupted runs reconciled."
                : "Interrupted runs reconciliation finished. Remaining items may still need operator recovery."
        }
    }

    private func clearOldRuns() {
        maintenanceInFlight = true
        maintenanceNotice = "Clearing old terminal runs."

        Task { @MainActor in
            do {
                let repository = RunRepository(context: modelContext)
                let cleanupPlan = try repository.prepareTerminalRunCleanup()

                if let selectedRunID, cleanupPlan.deletedRunIDs.contains(selectedRunID) {
                    self.selectedRunID = nil
                }

                let removedDirectoryCount = await RunRepository.removeFilesystemRoots(cleanupPlan)
                if cleanupPlan.deletedRunCount == 0, cleanupPlan.protectedRunCount == 0 {
                    maintenanceNotice = "No terminal runs needed cleanup."
                } else {
                    var parts: [String] = []
                    if cleanupPlan.deletedRunCount > 0 {
                        parts.append("Cleared \(cleanupPlan.deletedRunCount) terminal runs")
                    }
                    if removedDirectoryCount > 0 {
                        parts.append("removed \(removedDirectoryCount) owned directories")
                    }
                    if cleanupPlan.migratedAttachmentCount > 0 {
                        parts.append("migrated \(cleanupPlan.migratedAttachmentCount) referenced attachments into idea workspaces")
                    }
                    if cleanupPlan.protectedRunCount > 0 {
                        parts.append("kept \(cleanupPlan.protectedRunCount) referenced runs because their ideas do not have a valid project directory")
                    }
                    maintenanceNotice = parts.joined(separator: "; ").capitalized + "."
                }
            } catch {
                maintenanceErrorMessage = error.localizedDescription
                showMaintenanceError = true
                maintenanceNotice = "Run cleanup failed."
            }

            maintenanceInFlight = false
        }
    }
}

@MainActor
func buildRunsHomePresentationEntries(
    for runs: [Run],
    statusProvider: (Run) -> RunStatus
) -> [RunsHomeView.PresentationEntry] {
    runs.map { run in
        let effectiveStatus = statusProvider(run)
        return RunsHomeView.PresentationEntry(run: run, effectiveStatus: effectiveStatus)
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
    @Environment(\.modelContext) private var modelContext
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings

    let run: Run
    let effectiveStatus: RunStatus
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
                Text(displayTitle)
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
                    text: effectiveStatusLabel,
                    color: statusColor,
                    size: .small,
                    accessibilityIdentifier: "run-row-status-\(sanitizedRunTitle)"
                )
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

            if let onOpenGate, effectiveStatus == .waitingApproval {
                Button("Open Gate", systemImage: "checkmark.seal") { onOpenGate() }
            }

            if let onRecover, effectiveStatus == .blocked || effectiveStatus == .failed {
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
        switch effectiveStatus {
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

    private var effectiveStatusLabel: String {
        effectiveStatus.rawValue.replacingOccurrences(of: "_", with: " ")
    }

    private var sanitizedRunTitle: String {
        displayTitle
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
        var parts = [displayTitle, effectiveStatusLabel]
        parts.append(elapsedTimeString)
        return parts.joined(separator: ", ")
    }

    private var displayTitle: String {
        run.workflowTitle.isEmpty ? "Unknown Idea" : run.workflowTitle
    }

    private var elapsedTimeString: String {
        let elapsed = (run.completedAt ?? Date()).timeIntervalSince(run.startedAt)
        let mins = Int(elapsed) / 60
        let secs = Int(elapsed) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
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

    private var presentation: RuntimeTrustPresentation {
        RuntimeTrustPresentation(trustLevel: trustLevel)
    }

    var body: some View {
        StatusCapsule(
            text: presentation.badgeLabel,
            color: badgeColor,
            icon: presentation.badgeIcon,
            size: .small
        )
    }

    private var badgeColor: Color {
        switch presentation.badgeColorName {
        case "success":
            return DesignTokens.Status.success
        case "warning":
            return DesignTokens.Status.warning
        default:
            return DesignTokens.Status.neutral
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
    @Environment(ExecutionService.self) private var executionService
    @Environment(\.modelContext) private var modelContext
    let run: Run
    let onRecover: () -> Void
    let onCompare: () -> Void
    let onViewReport: () -> Void
    // Proposal 008 (§7.2): Blocked run recovery deep-link
    var onBlockedRecovery: (() -> Void)?
    let compatibilityChecker: CompatibilityChecker

    @State private var selectedPane: RunsHomePane = .summary
    @State private var selectedFlowSection: WorkflowMapVisibleSection = .topology
    @State private var selectedArtifactLeaf: RunArtifactLeaf?
    @State private var showTimelineInspector = false
    @State private var evidenceExportMessage: String?
    @State private var showStopConfirmation = false

    private var artifactHierarchy: RunArtifactHierarchy {
        RunArtifactHierarchyBuilder().build(for: run)
    }

    private var workflowMapProjection: WorkflowMapProjection? {
        WorkflowMapProjectionService(modelContext: modelContext, executionService: executionService)
            .projection(for: run)
    }

    private var effectiveRunStatus: RunStatus {
        workflowMapProjection?.runStatus ?? run.presentationStatus
    }

    private var effectiveRunStatusLabel: String {
        effectiveRunStatus.rawValue.replacingOccurrences(of: "_", with: " ")
    }

    private var effectiveCurrentStageLabel: String {
        workflowMapProjection?.currentStageLabel ?? run.cursorDerivedStageLabel
    }

    private var nextActionText: String {
        switch effectiveRunStatus {
        case .waitingApproval:
            return "Review approval context and decide whether this run should proceed."
        case .blocked:
            return run.driftDetails ?? "Inspect the blocked stage and choose recovery or comparison."
        case .failed:
            return "Inspect recovery evidence, compare against a compatible run, or reopen the report."
        case .completed:
            return "Review the report, exported evidence, and promoted artifacts."
        case .running, .pending, .ready:
            return "Watch flow progress, open the live timeline when needed, and inspect stage artifacts."
        case .cancelled:
            return "This run has settled after cancellation. Reports and artifacts remain available."
        case .cancelling:
            return "Cancellation is in progress. Operator evidence remains available while agents settle."
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
                    headerBlock

                    if hasAnyAction {
                        actionRow
                    }

                    Picker("Run Surface", selection: $selectedPane) {
                        ForEach(RunsHomePane.allCases, id: \.self) { pane in
                            Text(pane.title).tag(pane)
                        }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("runs-home-pane-picker")

                    switch selectedPane {
                    case .summary:
                        summaryPane
                    case .flow:
                        flowPane
                    case .artifacts:
                        artifactsPane
                    case .diagnostics:
                        diagnosticsPane
                    }
                }
                .padding()
            }
        }
        .navigationTitle("Run Details")
        .accessibilityIdentifier("run-detail-panel")
        .task(id: run.id) {
            selectedPane = defaultRunsHomePane(for: effectiveRunStatus)
        }
        .sheet(isPresented: $showTimelineInspector) {
            NavigationStack {
                if let projection = workflowMapProjection {
                    RunTimelineInspectorView(projection: projection)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Done") { showTimelineInspector = false }
                            }
                        }
                } else {
                    ContentUnavailableView(
                        "Live Timeline Unavailable",
                        systemImage: "waveform.path.ecg",
                        description: Text("The run could not be rebuilt into a workflow-map projection.")
                    )
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Done") { showTimelineInspector = false }
                        }
                    }
                }
            }
            .frame(minWidth: 620, minHeight: 520)
        }
        .sheet(item: $selectedArtifactLeaf) { leaf in
            RunArtifactLeafInspectorSheet(run: run, leaf: leaf)
        }
        .alert("Stop Run?", isPresented: $showStopConfirmation) {
            Button("Stop", role: .destructive) {
                Task {
                    await executionService.cancelRun(runID: run.id)
                }
            }
            Button("Keep Running", role: .cancel) { }
        } message: {
            Text("This will stop all active agents for \"\(run.idea?.title ?? run.workflowTitle)\". Run history and artifacts remain visible as terminal history.")
        }
    }

    @ViewBuilder
    private var headerBlock: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
            Text(run.idea?.title ?? "Unknown Idea")
                .font(.title2.bold())
            Text(run.workflowTitle)
                .font(.title3)
                .foregroundStyle(.secondary)
            HStack(alignment: .firstTextBaseline, spacing: DesignTokens.Spacing.small) {
                StatusCapsule(
                    text: effectiveRunStatusLabel,
                    color: statusColor,
                    size: .regular
                )
                RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
            }
            ParentIdeaArchiveBadge(title: "Parent idea", idea: run.idea)

            Text(nextActionText)
                .font(.caption)
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("runs-home-next-action-text")
        }
    }

    @ViewBuilder
    private var actionRow: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
            HStack(alignment: .center, spacing: DesignTokens.Spacing.small) {
                if run.canBeCancelledByOperator {
                    Button(
                        run.cancellationRequestedAt != nil ? "Cancelling\u{2026}" : "Stop Run",
                        systemImage: run.cancellationRequestedAt != nil ? "hourglass" : "stop.fill"
                    ) {
                        showStopConfirmation = true
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(DesignTokens.Status.error)
                    .disabled(run.cancellationRequestedAt != nil)
                    .accessibilityIdentifier("runs-home-stop-run-button")
                }

                if effectiveRunStatus == .blocked || effectiveRunStatus == .failed {
                    Button("Recover", systemImage: "arrow.counterclockwise") {
                        onRecover()
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(DesignTokens.Action.caution)

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

                if (run.status == .completed || run.status == .failed),
                   run.deliveryConfigurationJSON != nil {
                    Button("Export Evidence Pack", systemImage: "shippingbox") {
                        exportEvidencePack()
                    }
                    .buttonStyle(.bordered)
                    .accessibilityIdentifier("export-evidence-pack-button")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            if let evidenceExportMessage {
                Text(evidenceExportMessage)
                    .font(DesignTokens.Typography.supporting)
                    .foregroundStyle(.secondary)
                    .transition(.opacity)
            }
        }
    }

    @ViewBuilder
    private var summaryPane: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
            GroupBox("Summary") {
                VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
                    LabeledContent("Status", value: effectiveRunStatusLabel)
                    LabeledContent("Current Stage", value: effectiveCurrentStageLabel)
                    LabeledContent("Next Action", value: nextActionText)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Run Snapshot") {
                VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
                    LabeledContent("Workflow", value: run.workflowTitle)
                    if let idea = run.idea {
                        LabeledContent("Idea", value: idea.title)
                    }
                    LabeledContent("Current Stage", value: effectiveCurrentStageLabel)
                    LabeledContent("Status", value: effectiveRunStatusLabel)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    @ViewBuilder
    private var flowPane: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.medium) {
            Picker("Workflow Map Focus", selection: $selectedFlowSection) {
                ForEach(flowSections, id: \.self) { section in
                    Text(flowSectionTitle(section)).tag(section)
                }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("runs-home-flow-section-picker")

            WorkflowMapView(
                run: run,
                showsSummaryStrip: true,
                visibleSections: [selectedFlowSection],
                onOpenTimelineInspector: { showTimelineInspector = true }
            )

            Button("Open Live Timeline", systemImage: "waveform.path.ecg") {
                showTimelineInspector = true
            }
            .buttonStyle(.bordered)
            .accessibilityIdentifier("runs-home-open-timeline-button")
        }
    }

    @ViewBuilder
    private var artifactsPane: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.medium) {
            GroupBox("Artifact Hierarchy") {
                RunArtifactHierarchyView(
                    hierarchy: artifactHierarchy,
                    onOpenArtifact: { artifact in
                        selectedArtifactLeaf = resolvedLeaf(for: artifact)
                    },
                    artifactResolver: resolveArtifact(withID:)
                )
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    @ViewBuilder
    private var diagnosticsPane: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.large) {
            GroupBox("Execution Details") {
                VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
                    LabeledContent("Started", value: run.startedAt.formatted())
                    if let completed = run.completedAt {
                        LabeledContent("Completed", value: completed.formatted())
                    }
                    LabeledContent("Elapsed", value: elapsedTimeString)
                    if let cost = run.totalCostCents {
                        LabeledContent("Total Cost", value: "\(cost) cents")
                    }
                    if run.runtimeTrustLevel != nil {
                        LabeledContent("Runtime Trust", value: run.runtimeTrustDisplayLabel)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Repository & Delivery") {
                VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
                    if let worktreeRoot = run.worktreeRoot {
                        LabeledContent("Worktree", value: worktreeRoot)
                    }
                    if let repoIdentifier = run.repoIdentifier {
                        LabeledContent("Repository", value: repoIdentifier)
                    }
                    if let baseBranch = run.baseBranch {
                        LabeledContent("Base Branch", value: baseBranch)
                    }
                    if let targetBranch = run.targetBranch {
                        LabeledContent("Target Branch", value: targetBranch)
                    }
                    if let releaseTargetID = run.releaseTargetID {
                        LabeledContent("Release Target", value: releaseTargetID)
                    }
                    if let releaseMode = run.releaseMode {
                        LabeledContent("Release Mode", value: releaseMode)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Receipts & Evidence") {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(diagnosticArtifacts) { artifact in
                        Button {
                            selectedArtifactLeaf = artifact
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(artifact.name)
                                    Text("\(artifact.stageLabel) · \(artifact.agentTitle)")
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

                    Button("Open Live Timeline", systemImage: "waveform.path.ecg") {
                        showTimelineInspector = true
                    }
                    .buttonStyle(.bordered)
                }
            }
        }
    }

    private var hasAnyAction: Bool {
        run.canBeCancelledByOperator
            || run.presentationStatus == .blocked || run.presentationStatus == .failed
            || compatibilityChecker.hasCompatibleTargets(for: run)
            || run.latestImmutableReportArtifactID != nil
            || (run.deliveryConfigurationJSON != nil
                && (run.presentationStatus == .completed || run.presentationStatus == .failed))
    }

    private var flowSections: [WorkflowMapVisibleSection] {
        WorkflowMapVisibleSection.allCases.filter { $0 != .timeline }
    }

    private func flowSectionTitle(_ section: WorkflowMapVisibleSection) -> String {
        switch section {
        case .topology:
            return "Topology"
        case .handoffs:
            return "Handoffs"
        case .agents:
            return "Agents"
        case .telemetry:
            return "Telemetry"
        case .timeline:
            return "Timeline"
        }
    }

    private var diagnosticArtifacts: [RunArtifactLeaf] {
        let relevantKinds: Set<RunArtifactBucketKind> = [.receipt, .transcript, .review, .diagnostic, .report, .release, .delivery]
        return artifactHierarchy.allArtifacts.filter { leaf in
            relevantKinds.contains(classify(leaf: leaf))
                || leaf.isLatestSummaryReport
                || leaf.isLatestImmutableReport
                || leaf.name.contains("evidence")
                || leaf.name.contains("recovery")
        }
    }

    private func classify(leaf: RunArtifactLeaf) -> RunArtifactBucketKind {
        let name = leaf.name.lowercased()
        let contractID = leaf.contractID.lowercased()
        let displayRole = leaf.artifactLineageKind?.lowercased() ?? ""

        if leaf.reportKind == "latest_summary" || name.contains("summary") || displayRole.contains("summary") {
            return .summary
        }
        if leaf.reportKind != nil || contractID.contains("run_report") {
            return .report
        }
        if name.contains("diff") || name.contains("patch") {
            return .diff
        }
        if name.contains("receipt") || contractID.contains("receipt") {
            return .receipt
        }
        if name.contains("transcript") {
            return .transcript
        }
        if name.contains("approval") {
            return .approvalContext
        }
        if name.contains("review") {
            return .review
        }
        if name.contains("release") || name.contains("manifest") || contractID.contains("release") {
            return .release
        }
        if name.contains("delivery") || name.contains("publish") || name.contains("upload") {
            return .delivery
        }
        if name.contains("test") || contractID.contains("test") {
            return .test
        }
        if name.contains("diagnostic") || name.contains("trace") || name.contains("debug") || name.contains("log") {
            return .diagnostic
        }
        return .other
    }

    private func resolveArtifact(withID artifactID: UUID) -> Artifact? {
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.id == artifactID
            }
        )
        return try? modelContext.fetch(descriptor).first
    }

    private func resolvedLeaf(for artifact: Artifact) -> RunArtifactLeaf? {
        artifactHierarchy.allArtifacts.first { $0.artifactID == artifact.id }
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
            if let warning = markBenchmarkRecordExported() {
                evidenceExportMessage = "\(evidenceExportMessage ?? "Export completed.") Warning: \(warning)"
            }
        } catch {
            evidenceExportMessage = "Export failed: \(error.localizedDescription)"
        }
    }

    /// Proposal 008 (REQ-020): Stamp the benchmark execution record with the export timestamp.
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

    // Proposal 012 (M-02): Semantic status colors
    private var statusColor: Color {
        switch effectiveRunStatus {
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

private struct RunArtifactLeafInspectorSheet: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss

    let run: Run
    let leaf: RunArtifactLeaf

    var body: some View {
        NavigationStack {
            if let artifact = artifact {
                ArtifactInspectorView(artifact: artifact, run: run)
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Done") { dismiss() }
                        }
                    }
            } else {
                ContentUnavailableView(
                    "Artifact Unavailable",
                    systemImage: "doc.questionmark",
                    description: Text("The selected artifact could not be loaded from the current run.")
                )
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Done") { dismiss() }
                    }
                }
            }
        }
        .frame(minWidth: 960, minHeight: 640)
    }

    private var artifact: Artifact? {
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.id == leaf.artifactID
            }
        )
        return try? modelContext.fetch(descriptor).first
    }
}

private extension RunsHomePane {
    var title: String {
        switch self {
        case .summary:
            return "Summary"
        case .flow:
            return "Flow"
        case .artifacts:
            return "Artifacts"
        case .diagnostics:
            return "Diagnostics"
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
