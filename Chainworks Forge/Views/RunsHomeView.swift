import SwiftUI
import SwiftData

// MARK: - P005-OPS §5: Runs Home View

/// Primary operator landing surface.
/// Answers: "What needs my attention right now, and what safe action is available?"
/// Runs grouped into: Waiting Approval, Blocked, Running, Recently Completed.
struct RunsHomeView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService

    @Query(sort: \Run.startedAt, order: .reverse)
    private var allRuns: [Run]

    @State private var selectedRun: Run?
    @State private var showRecoverySheet = false
    @State private var showComparisonPicker = false
    @State private var comparisonTargetRun: Run?
    @State private var showReportView = false

    var body: some View {
        NavigationSplitView {
            List(selection: $selectedRun) {
                // §5.2: Waiting Approval
                if !waitingApprovalRuns.isEmpty {
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
                            .foregroundStyle(.orange)
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
                            .foregroundStyle(.red)
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
                            .foregroundStyle(.blue)
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
                            .foregroundStyle(.green)
                    }
                }

                if allRuns.isEmpty {
                    ContentUnavailableView(
                        "No Runs",
                        systemImage: "tray",
                        description: Text("Start a run from the Ideas tab to see it here.")
                    )
                }
            }
            .navigationTitle("Runs Home")
            .accessibilityIdentifier("runs-home-list")
        } detail: {
            if let run = selectedRun {
                RunDetailPanel(
                    run: run,
                    onRecover: { showRecoverySheet = true },
                    onCompare: { showComparisonPicker = true },
                    onViewReport: { showReportView = true },
                    compatibilityChecker: compatibilityChecker
                )
            } else {
                ContentUnavailableView(
                    "Select a Run",
                    systemImage: "sidebar.left",
                    description: Text("Choose a run from the sidebar to view details.")
                )
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
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(run.idea?.title ?? "Unknown Idea")
                    .font(.headline)
                Spacer()
                Image(systemName: attentionLevel.icon)
                    .foregroundStyle(attentionLevel.color)
            }

            HStack(spacing: 8) {
                Text(run.workflowTitle)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Divider().frame(height: 12)
                Text(run.status.rawValue)
                    .font(.caption)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(statusColor.opacity(0.15))
                    .foregroundStyle(statusColor)
                    .clipShape(Capsule())
                if let stageLabel = currentStageLabel {
                    Text(stageLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            HStack(spacing: 12) {
                Label(elapsedTimeString, systemImage: "clock")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                if let cost = run.totalCostCents {
                    Label("\(cost)c", systemImage: "dollarsign.circle")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Label(lastProgressString, systemImage: "arrow.clockwise")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Spacer()
                RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
            }
        }
        .padding(.vertical, 4)
        .accessibilityIdentifier("run-row-\(run.id.uuidString)")
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

    private var statusColor: Color {
        switch run.status {
        case .completed: return .green
        case .failed: return .red
        case .blocked: return .red
        case .waitingApproval: return .orange
        case .running: return .blue
        case .cancelled: return .gray
        case .pending, .ready: return .secondary
        }
    }

    private var currentStageLabel: String? {
        guard let stageID = run.currentStageID else { return nil }
        return run.stageExecutions.first(where: { $0.stageID == stageID })?.label
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

struct RuntimeProvenanceBadge: View {
    let trustLevel: String?

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: badgeIcon)
                .font(.caption2)
            Text(badgeLabel)
                .font(.caption2)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .background(badgeColor.opacity(0.12))
        .foregroundStyle(badgeColor)
        .clipShape(Capsule())
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
        case "fixture_verified": return .green
        case "server_verified": return .green
        case "server_unverified": return .orange
        default: return .secondary
        }
    }
}

// MARK: - Run Detail Panel

struct RunDetailPanel: View {
    let run: Run
    let onRecover: () -> Void
    let onCompare: () -> Void
    let onViewReport: () -> Void
    let compatibilityChecker: CompatibilityChecker

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(run.idea?.title ?? "Unknown Idea")
                        .font(.title)
                    Text(run.workflowTitle)
                        .font(.title3)
                        .foregroundStyle(.secondary)
                    HStack {
                        Text(run.status.rawValue)
                            .font(.headline)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(statusColor.opacity(0.15))
                            .foregroundStyle(statusColor)
                            .clipShape(Capsule())
                        RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
                    }
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
                    .font(.headline)
                ForEach(run.stageExecutions.sorted(by: { $0.startedAt < $1.startedAt })) { stage in
                    HStack {
                        Image(systemName: stageIcon(stage.status))
                            .foregroundStyle(stageColor(stage.status))
                        Text(stage.label)
                        Spacer()
                        Text(stage.status.rawValue)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Divider()

                Text("Workflow Map")
                    .font(.headline)
                WorkflowMapView(run: run)

                Divider()

                // §5.4: Contextual actions — only executable actions
                HStack(spacing: 12) {
                    if run.status == .blocked || run.status == .failed {
                        Button("Recover", systemImage: "arrow.counterclockwise") {
                            onRecover()
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(.orange)
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
                }
            }
            .padding()
        }
        .navigationTitle("Run Details")
    }

    private var statusColor: Color {
        switch run.status {
        case .completed: return .green
        case .failed, .blocked: return .red
        case .waitingApproval: return .orange
        case .running: return .blue
        default: return .secondary
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
        case .completed: return .green
        case .failed: return .red
        case .running: return .blue
        case .waitingApproval: return .orange
        case .blocked: return .red
        case .skipped: return .gray
        case .pending, .ready: return .secondary
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
                                    Text(target.status.rawValue)
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
