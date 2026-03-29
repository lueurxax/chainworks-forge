import SwiftUI
import SwiftData

// MARK: - BlockedRunRecoveryView (Proposal 008 — §7.2, ARCH-085)

/// Shell-owned subview for blocked implementation/review/release re-entry.
/// Entered from RunsHomeView, hosted inside the shell hierarchy.
/// NOT a parallel top-level destination.
///
/// Shows the blocker reason, stage history timeline, preserved receipts,
/// diff/test context (when available), and the next valid operator actions.
struct BlockedRunRecoveryView: View {
    let run: Run
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss
    @State private var recoveryPath: RecoveryPath = .undetermined
    @State private var isExecuting = false
    @State private var errorMessage: String?
    @State private var receiptArtifacts: [Artifact] = []
    @State private var diffArtifacts: [Artifact] = []
    @State private var testArtifacts: [Artifact] = []
    // Proposal 013: Evidence-aware recovery
    @State private var evidencePacket: FailedStageEvidencePacket?
    @State private var showEvidencePanel = false

    // MARK: - Recovery Path Classification

    enum RecoveryPath: String {
        case resume = "Resume"
        case retry = "Retry"
        case clone = "Clone"
        case cancel = "Cancel"
        case undetermined = "Evaluating"

        var icon: String {
            switch self {
            case .resume: return "play.circle.fill"
            case .retry: return "arrow.counterclockwise.circle.fill"
            case .clone: return "doc.on.doc.fill"
            case .cancel: return "xmark.circle.fill"
            case .undetermined: return "questionmark.circle"
            }
        }

        var tint: Color {
            switch self {
            case .resume: return .green
            case .retry: return .orange
            case .clone: return .blue
            case .cancel: return .red
            case .undetermined: return .secondary
            }
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // 1. Blocker summary header
                blockerSummarySection

                Divider()

                // 2. Recovery path indicator
                recoveryPathIndicator

                Divider()

                // 3. Stage history timeline
                stageHistorySection

                // 4. Preserved receipts
                if !receiptArtifacts.isEmpty {
                    receiptsSection
                }

                // 5. Diff / test context
                if !diffArtifacts.isEmpty || !testArtifacts.isEmpty {
                    diffTestContextSection
                }

                Divider()

                // 6. Recovery actions
                recoveryActionsSection

                // Error display
                if let errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.top, 4)
                }
            }
            .padding()
        }
        .navigationTitle("Run Recovery")
        .task {
            loadArtifactContext()
            loadEvidencePacket()
            classifyRecoveryPath()
        }
        .sheet(isPresented: $showEvidencePanel) {
            if let packet = evidencePacket {
                NavigationStack {
                    ScrollView {
                        FailedStageEvidencePanel(evidencePacket: packet)
                    }
                    .navigationTitle("Failure Evidence")
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Close") { showEvidencePanel = false }
                        }
                    }
                }
                .frame(minWidth: 500, minHeight: 400)
            }
        }
    }

    // MARK: - Blocker Summary (§7.2)

    private var blockerSummarySection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 10) {
                    Image(systemName: blockerIcon)
                        .font(.title2)
                        .foregroundStyle(blockerColor)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(run.idea?.title ?? "Unknown Idea")
                            .font(.title3.bold())
                        Text(run.workflowTitle)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    statusCapsule
                }

                Divider()

                VStack(alignment: .leading, spacing: 4) {
                    Label("Blocker Reason", systemImage: "exclamationmark.bubble.fill")
                        .font(.caption.bold())
                        .foregroundStyle(.secondary)
                    Text(blockerReason)
                        .font(.body)
                        .textSelection(.enabled)
                }

                HStack(spacing: 16) {
                    LabeledContent("Run ID") {
                        Text(run.id.uuidString.prefix(8))
                            .font(.caption.monospaced())
                    }
                    LabeledContent("Current Stage") {
                        Text(currentStageName)
                            .font(.caption.monospaced())
                    }
                    if let cost = run.totalCostCents {
                        LabeledContent("Cost") {
                            Text("\(cost)c")
                                .font(.caption.monospaced())
                        }
                    }
                }
                .font(.caption)

                HStack(spacing: 8) {
                    RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
                    ParentIdeaArchiveBadge(title: "Parent idea", idea: run.idea)
                    // Proposal 013: Evidence panel button
                    if evidencePacket != nil {
                        Button {
                            showEvidencePanel = true
                        } label: {
                            Label("Failure Evidence", systemImage: "doc.text.magnifyingglass")
                                .font(.caption)
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }
                }
            }
        } label: {
            Label("Blocker Summary", systemImage: "exclamationmark.triangle.fill")
                .foregroundStyle(blockerColor)
        }
    }

    // MARK: - Recovery Path Indicator

    private var recoveryPathIndicator: some View {
        GroupBox {
            HStack(spacing: 12) {
                Image(systemName: recoveryPath.icon)
                    .font(.title2)
                    .foregroundStyle(recoveryPath.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Recovery Path: \(recoveryPath.rawValue)")
                        .font(.headline)
                    Text(recoveryPathDescription)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }
        } label: {
            Label("Recommended Path", systemImage: "arrow.triangle.turn.up.right.diamond.fill")
        }
    }

    // MARK: - Stage History Timeline (§7.2)

    private var stageHistorySection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 0) {
                let sortedStages = run.stageExecutions.sorted { $0.startedAt < $1.startedAt }
                if sortedStages.isEmpty {
                    Text("No stage history available.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(Array(sortedStages.enumerated()), id: \.element.id) { index, stage in
                        HStack(alignment: .top, spacing: 12) {
                            // Timeline connector
                            VStack(spacing: 0) {
                                Circle()
                                    .fill(stageColor(stage.status))
                                    .frame(width: 10, height: 10)
                                if index < sortedStages.count - 1 {
                                    Rectangle()
                                        .fill(Color.secondary.opacity(0.3))
                                        .frame(width: 2)
                                        .frame(maxHeight: .infinity)
                                }
                            }
                            .frame(width: 10)

                            VStack(alignment: .leading, spacing: 2) {
                                HStack {
                                    Image(systemName: stageIcon(stage.status))
                                        .foregroundStyle(stageColor(stage.status))
                                        .font(.caption)
                                    Text(stage.label)
                                        .font(.subheadline.bold())
                                    Spacer()
                                    Text(stage.status.rawValue)
                                        .font(.caption2)
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(stageColor(stage.status).opacity(0.15))
                                        .foregroundStyle(stageColor(stage.status))
                                        .clipShape(Capsule())
                                }
                                HStack(spacing: 12) {
                                    Text(stage.stageID)
                                        .font(.caption2.monospaced())
                                        .foregroundStyle(.secondary)
                                    Text("Attempt #\(stage.attemptNumber)")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                    Text(stage.startedAt.formatted(.dateTime.hour().minute().second()))
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                }
                                // Show agent details for blocked/failed stages
                                if stage.status == .blocked || stage.status == .failed {
                                    ForEach(stage.agentExecutions.filter { $0.status == .failed || $0.status == .cancelled }) { agent in
                                        HStack(spacing: 4) {
                                            Image(systemName: "person.circle")
                                                .font(.caption2)
                                                .foregroundStyle(.red)
                                            Text("\(agent.agentTitle): \(agent.logSnippet ?? "no detail")")
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                                .lineLimit(2)
                                        }
                                        .padding(.leading, 4)
                                    }
                                }
                            }
                        }
                        .padding(.vertical, 6)
                    }
                }
            }
        } label: {
            Label("Stage History", systemImage: "clock.arrow.circlepath")
        }
    }

    // MARK: - Preserved Receipts (§7.2)

    private var receiptsSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 6) {
                ForEach(receiptArtifacts) { artifact in
                    HStack(spacing: 8) {
                        Image(systemName: "doc.seal.fill")
                            .foregroundStyle(.green)
                            .font(.caption)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(artifact.name)
                                .font(.caption.monospaced())
                            HStack(spacing: 8) {
                                Text(artifact.stageID)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                Text(artifact.agentID)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                if let checksum = artifact.checksumSHA256 {
                                    Text(checksum.prefix(12) + "...")
                                        .font(.caption2.monospaced())
                                        .foregroundStyle(.tertiary)
                                }
                            }
                        }
                        Spacer()
                        Text(artifact.format.rawValue)
                            .font(.caption2)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(Color.secondary.opacity(0.1))
                            .clipShape(Capsule())
                    }
                }
            }
        } label: {
            Label("Preserved Receipts (\(receiptArtifacts.count))", systemImage: "doc.seal")
        }
    }

    // MARK: - Diff / Test Context

    private var diffTestContextSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                if !diffArtifacts.isEmpty {
                    DisclosureGroup {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(diffArtifacts) { artifact in
                                HStack(spacing: 6) {
                                    Image(systemName: "chevron.left.forwardslash.chevron.right")
                                        .font(.caption2)
                                        .foregroundStyle(.blue)
                                    Text(artifact.name)
                                        .font(.caption.monospaced())
                                    Spacer()
                                    Text(artifact.stageID)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    } label: {
                        Label("Diff Artifacts (\(diffArtifacts.count))", systemImage: "arrow.left.arrow.right")
                            .font(.subheadline)
                    }
                }
                if !testArtifacts.isEmpty {
                    DisclosureGroup {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(testArtifacts) { artifact in
                                HStack(spacing: 6) {
                                    Image(systemName: "testtube.2")
                                        .font(.caption2)
                                        .foregroundStyle(.purple)
                                    Text(artifact.name)
                                        .font(.caption.monospaced())
                                    Spacer()
                                    Text(artifact.stageID)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    } label: {
                        Label("Test Artifacts (\(testArtifacts.count))", systemImage: "testtube.2")
                            .font(.subheadline)
                    }
                }
            }
        } label: {
            Label("Context", systemImage: "doc.text.magnifyingglass")
        }
    }

    // MARK: - Recovery Actions (§7.2)

    private var recoveryActionsSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 10) {
                let actions = availableRecoveryActions

                if actions.isEmpty {
                    HStack {
                        Image(systemName: "nosign")
                            .foregroundStyle(.secondary)
                        Text("No recovery actions available for this run state.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                } else {
                    ForEach(actions) { action in
                        HStack(spacing: 12) {
                            Image(systemName: action.systemImage)
                                .font(.title3)
                                .foregroundStyle(actionColor(action))
                                .frame(width: 28)

                            VStack(alignment: .leading, spacing: 2) {
                                Text(action.label)
                                    .font(.subheadline.bold())
                                Text(actionDescription(action))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }

                            Spacer()

                            Button {
                                executeRecoveryAction(action)
                            } label: {
                                Text("Execute")
                            }
                            .buttonStyle(.bordered)
                            .tint(actionColor(action))
                            .disabled(isExecuting)
                        }
                        .padding(.vertical, 4)
                    }

                    Divider()

                    // Cancel run -- always available for non-terminal runs
                    if !isTerminalStatus(run.status) {
                        HStack(spacing: 12) {
                            Image(systemName: "xmark.circle.fill")
                                .font(.title3)
                                .foregroundStyle(.red)
                                .frame(width: 28)

                            VStack(alignment: .leading, spacing: 2) {
                                Text("Cancel Run")
                                    .font(.subheadline.bold())
                                Text("Permanently halt this run. Cannot be undone.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }

                            Spacer()

                            Button(role: .destructive) {
                                cancelRun()
                            } label: {
                                Text("Cancel")
                            }
                            .buttonStyle(.bordered)
                            .disabled(isExecuting)
                        }
                        .padding(.vertical, 4)
                    }
                }
            }
        } label: {
            Label("Recovery Actions", systemImage: "wrench.and.screwdriver")
        }
    }

    // MARK: - Computed Properties

    private var blockerReason: String {
        if let details = run.driftDetails {
            return details
        }

        // Derive from stage status
        let failedStages = run.stageExecutions.filter { $0.status == .failed }
        let blockedStages = run.stageExecutions.filter { $0.status == .blocked }

        if !failedStages.isEmpty {
            let stageNames = failedStages.map(\.label).joined(separator: ", ")
            return "Stage failure in: \(stageNames)"
        }
        if !blockedStages.isEmpty {
            let stageNames = blockedStages.map(\.label).joined(separator: ", ")
            return "Blocked at: \(stageNames)"
        }

        switch run.status {
        case .waitingApproval:
            return "Run is waiting for operator approval at an approval gate."
        case .cancelled:
            return "Run was cancelled by operator."
        case .failed:
            return "Run failed during execution."
        default:
            return "Run cannot proceed. Review stage history for details."
        }
    }

    private var blockerIcon: String {
        switch run.status {
        case .failed: return "xmark.octagon.fill"
        case .blocked: return "exclamationmark.triangle.fill"
        case .waitingApproval: return "pause.circle.fill"
        case .cancelled: return "stop.circle.fill"
        default: return "questionmark.circle.fill"
        }
    }

    private var blockerColor: Color {
        switch run.status {
        case .failed, .blocked: return .red
        case .waitingApproval: return .orange
        case .cancelled: return .gray
        default: return .secondary
        }
    }

    private var currentStageName: String {
        guard let stageID = run.currentStageID else { return "None" }
        return run.stageExecutions.first(where: { $0.stageID == stageID })?.label ?? stageID
    }

    private var statusCapsule: some View {
        Text(run.presentationStatusLabel)
            .font(.caption.bold())
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(blockerColor.opacity(0.15))
            .foregroundStyle(blockerColor)
            .clipShape(Capsule())
    }

    private var recoveryPathDescription: String {
        switch recoveryPath {
        case .resume:
            return "The run can resume from its current approval gate."
        case .retry:
            return "The failed or blocked stage can be retried from the last checkpoint."
        case .clone:
            return "The run should be cloned with frozen or current configuration."
        case .cancel:
            return "No viable recovery path. Consider cancelling and starting fresh."
        case .undetermined:
            return "Evaluating available recovery options..."
        }
    }

    private var availableRecoveryActions: [RecoveryAction] {
        let coordinator = RecoveryCoordinator(modelContext: modelContext)
        return coordinator.availableActions(for: run)
    }

    // MARK: - Data Loading

    // Proposal 013: Load and present evidence packet
    private func loadEvidencePacket() {
        let coordinator = RecoveryCoordinator(modelContext: modelContext)
        evidencePacket = coordinator.buildEvidencePacket(for: run)
    }

    private func loadArtifactContext() {
        let runID = run.id
        let allArtifacts: [Artifact]

        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID
            },
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        allArtifacts = (try? modelContext.fetch(descriptor)) ?? []

        // Receipts: artifacts with "receipt" in the name
        receiptArtifacts = allArtifacts.filter { artifact in
            artifact.name.localizedCaseInsensitiveContains("receipt")
        }

        // Diff artifacts
        diffArtifacts = allArtifacts.filter { artifact in
            artifact.format == .diff
            || artifact.name.localizedCaseInsensitiveContains("diff")
            || artifact.name.localizedCaseInsensitiveContains("changed_files")
        }

        // Test artifacts
        testArtifacts = allArtifacts.filter { artifact in
            artifact.name.localizedCaseInsensitiveContains("test")
        }
    }

    private func classifyRecoveryPath() {
        switch run.status {
        case .waitingApproval:
            recoveryPath = .resume
        case .failed:
            let hasRetryableStage = run.stageExecutions.contains { $0.status == .failed }
            recoveryPath = hasRetryableStage ? .retry : .clone
        case .blocked:
            let hasRetryableStage = run.stageExecutions.contains { $0.status == .blocked || $0.status == .failed }
            recoveryPath = hasRetryableStage ? .retry : .clone
        case .cancelled:
            recoveryPath = .cancel
        default:
            recoveryPath = .undetermined
        }
    }

    // MARK: - Actions

    private func executeRecoveryAction(_ action: RecoveryAction) {
        isExecuting = true
        errorMessage = nil

        let coordinator = RecoveryCoordinator(modelContext: modelContext)

        do {
            switch action {
            case .retryAgent(let stageID, let agentID):
                _ = try coordinator.retryAgent(run: run, stageID: stageID, agentID: agentID)
                dismiss()

            case .retryStage(let stageID):
                _ = try coordinator.retryStage(run: run, stageID: stageID)
                dismiss()

            case .resumeFromApprovalGate(let stageID):
                _ = try coordinator.resumeFromApprovalGate(run: run, stageID: stageID)
                dismiss()

            case .cloneRunFrozenSnapshot:
                guard let idea = run.idea else {
                    errorMessage = "No idea associated with this run."
                    isExecuting = false
                    return
                }
                let compiler = RunPlanCompiler(modelContext: modelContext)
                _ = try coordinator.cloneRunFrozenSnapshot(
                    original: run,
                    idea: idea,
                    compiler: compiler
                )
                dismiss()

            case .cloneRunCurrentConfig:
                guard let idea = run.idea else {
                    errorMessage = "No idea associated with this run."
                    isExecuting = false
                    return
                }
                guard let workflow = try? YAMLParser.loadWorkflow(
                    from: URL(fileURLWithPath: run.workflowSourcePath)
                ) else {
                    errorMessage = "Cannot load workflow from source path."
                    isExecuting = false
                    return
                }
                // Catalog is required but not available here without ExecutionService.
                // This path defers to the parent shell for full clone-with-current orchestration.
                errorMessage = "Clone with current config requires orchestration from Runs Home."
                isExecuting = false
                return
            }
        } catch {
            errorMessage = error.localizedDescription
        }

        isExecuting = false
    }

    private func cancelRun() {
        isExecuting = true
        errorMessage = nil

        run.status = .cancelled
        run.completedAt = Date()
        run.cancellationRequestedAt = Date()
        run.cancellationSettledAt = Date()

        do {
            try modelContext.save()
            dismiss()
        } catch {
            errorMessage = "Failed to cancel run: \(error.localizedDescription)"
        }

        isExecuting = false
    }

    // MARK: - Helpers

    private func isTerminalStatus(_ status: RunStatus) -> Bool {
        [.completed, .failed, .cancelled].contains(status)
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

    private func actionColor(_ action: RecoveryAction) -> Color {
        switch action {
        case .resumeFromApprovalGate: return .green
        case .retryAgent, .retryStage: return .orange
        case .cloneRunFrozenSnapshot: return .blue
        case .cloneRunCurrentConfig: return .indigo
        }
    }

    private func actionDescription(_ action: RecoveryAction) -> String {
        switch action {
        case .resumeFromApprovalGate(let stageID):
            return "Re-arm the approval gate at \(stageID) and continue execution."
        case .retryAgent(let stageID, let agentID):
            return "Retry agent \(agentID) in stage \(stageID) from its last checkpoint."
        case .retryStage(let stageID):
            return "Reset all agents in stage \(stageID) and re-execute from the beginning."
        case .cloneRunFrozenSnapshot:
            return "Create a new run using the original frozen workflow and catalog snapshots."
        case .cloneRunCurrentConfig:
            return "Create a new run using the current workflow and catalog from disk."
        }
    }
}
