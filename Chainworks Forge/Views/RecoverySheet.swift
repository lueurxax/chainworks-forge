import SwiftUI
import SwiftData

// MARK: - P005-OPS §7.4 + Proposal 008 §7.2: Recovery Sheet

/// Surfaces blocked/failed run recovery with:
/// - reason, most recent stage, trust/provenance summary,
/// - suggested next safe action, list of allowed actions,
/// - Proposal 008 additions: preserved receipts, stage history, evidence-pack status.
/// Only exposes actions allowed for the current run type.
struct RecoverySheet: View {
    let run: Run
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Environment(\.dismiss) private var dismiss
    @State private var recoveryContext: RecoveryContext?
    @State private var isExecuting = false
    @State private var errorMessage: String?
    // Proposal 008 (§7.2): Show stage history and preserved receipts
    @State private var showStageHistory = false
    @State private var showPreservedReceipts = false
    // Proposal 013 (§7.3): Failed stage evidence panel
    @State private var showEvidencePanel = false
    @State private var evidencePacket: FailedStageEvidencePacket?

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                if let context = recoveryContext {
                    GroupBox("Blocked Reason") {
                        Text(context.reason)
                            .font(.body)
                    }

                    GroupBox("Most Recent Stage") {
                        Text(context.mostRecentStage)
                            .font(.headline)
                    }

                    GroupBox("Runtime Trust") {
                        VStack(alignment: .leading, spacing: 6) {
                            RuntimeProvenanceBadge(trustLevel: context.trustSummary)
                            if let bindingSummary = context.bindingSummary {
                                Text(bindingSummary)
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.secondary)
                                    .textSelection(.enabled)
                            }
                        }
                    }

                    // Proposal 013 (§7.3): Evidence summary
                    if let evidenceSummary = context.evidenceSummary {
                        GroupBox("Evidence") {
                            HStack {
                                Image(systemName: "doc.text.magnifyingglass")
                                    .foregroundStyle(.orange)
                                VStack(alignment: .leading) {
                                    Text(evidenceSummary)
                                        .font(.caption)
                                    if let failureClass = context.failureClass {
                                        Text("Failure: \(failureClass.replacingOccurrences(of: "_", with: " "))")
                                            .font(.caption2)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                Spacer()
                                if evidencePacket != nil {
                                    Button("Details") {
                                        showEvidencePanel = true
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.small)
                                }
                            }
                        }
                    }

                    Divider()

                    if let suggested = context.suggestedAction {
                        GroupBox("Suggested Action") {
                            HStack {
                                Image(systemName: suggested.systemImage)
                                    .font(.title3)
                                    .foregroundStyle(DesignTokens.Action.primary)
                                VStack(alignment: .leading) {
                                    Text(suggested.label)
                                        .font(.headline)
                                    Text("Recommended next step")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Button("Execute") {
                                    executeAction(suggested)
                                }
                                .buttonStyle(.borderedProminent)
                                .disabled(isExecuting)
                            }
                        }
                    }

                    GroupBox("All Available Actions") {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(context.allowedActions) { action in
                                HStack {
                                    Image(systemName: action.systemImage)
                                        .frame(width: 24)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(action.label)
                                            .font(.callout)
                                        // Proposal 013 §7.2: Action explanation text
                                        Text(recoveryActionExplanation(action))
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                    Spacer()
                                    Button("Execute") {
                                        executeAction(action)
                                    }
                                    .buttonStyle(.bordered)
                                    .disabled(isExecuting)
                                }
                            }
                        }
                    }

                    // Proposal 008 (§7.2): Stage history disclosure
                    DisclosureGroup("Stage History", isExpanded: $showStageHistory) {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(canonicalStageHistory) { stage in
                                HStack {
                                    Image(systemName: stageStatusIcon(stage.status))
                                        .foregroundStyle(stageStatusColor(stage.status))
                                        .frame(width: 20)
                                    Text(stage.label)
                                        .font(.caption)
                                    Spacer()
                                    Text(stage.status.rawValue)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                    if stage.attemptNumber > 1 {
                                        Text("(attempt \(stage.attemptNumber))")
                                            .font(.caption2)
                                            .foregroundStyle(.orange)
                                    }
                                }
                            }
                        }
                    }

                    // Proposal 008 (§7.2): Preserved receipts disclosure
                    let receiptArtifacts = run.stageExecutions
                        .flatMap(\.agentExecutions)
                        .flatMap(\.artifacts)
                        .filter { $0.name.contains("receipt") || $0.contractID == "provider_receipt" || $0.contractID == "delivery_receipt" }
                    if !receiptArtifacts.isEmpty {
                        DisclosureGroup("Preserved Receipts (\(receiptArtifacts.count))", isExpanded: $showPreservedReceipts) {
                            VStack(alignment: .leading, spacing: 4) {
                                ForEach(receiptArtifacts) { receipt in
                                    HStack {
                                        Image(systemName: "doc.text.fill")
                                            .foregroundStyle(DesignTokens.Action.primary)
                                            .frame(width: 20)
                                        VStack(alignment: .leading) {
                                            Text(receipt.name)
                                                .font(.caption)
                                            Text(receipt.createdAt.formatted())
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let error = errorMessage {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                            .padding(.horizontal)
                    }

                } else {
                    ProgressView("Loading recovery context...")
                }

                Spacer()
            }
            .padding()
            .navigationTitle("Recovery")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        // Proposal 012 (L-09): Escape to dismiss recovery sheet
                        .keyboardShortcut(.escape, modifiers: [])
                }
            }
        }
        .frame(minWidth: 500, minHeight: 400)
        .task {
            let coordinator = RecoveryCoordinator(modelContext: modelContext)
            recoveryContext = coordinator.recoveryContext(for: run)
            // Proposal 013: Build evidence packet for the evidence panel
            evidencePacket = coordinator.buildEvidencePacket(for: run)
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

    private func executeAction(_ action: RecoveryAction) {
        isExecuting = true
        errorMessage = nil

        let coordinator = RecoveryCoordinator(modelContext: modelContext)
        let compiler = RunPlanCompiler(modelContext: modelContext)

        do {
            switch action {
            case .resumeRun(let stageID):
                try executionService.resumeRun(run: run, compiler: compiler, stageID: stageID)
                dismiss()

            case .retryAgent(let stageID, let agentID):
                _ = try coordinator.retryAgent(run: run, stageID: stageID, agentID: agentID)
                try executionService.resumeRun(run: run, compiler: compiler, stageID: stageID)
                dismiss()

            case .retryAggregateStep(let stageID):
                _ = try coordinator.retryAggregateStep(run: run, stageID: stageID)
                try executionService.resumeRun(run: run, compiler: compiler, stageID: stageID)
                dismiss()

            case .retryStage(let stageID):
                _ = try coordinator.retryStage(run: run, stageID: stageID)
                try executionService.resumeRun(run: run, compiler: compiler, stageID: stageID)
                dismiss()

            case .resumeFromApprovalGate(let stageID):
                _ = try coordinator.resumeFromApprovalGate(run: run, stageID: stageID)
                try executionService.resumeRun(run: run, compiler: compiler, stageID: stageID)
                dismiss()

            case .cloneRunFrozenSnapshot:
                guard let idea = run.idea else {
                    errorMessage = "No idea associated with this run"
                    break
                }
                let compiler = RunPlanCompiler(modelContext: modelContext)
                let clone = try coordinator.cloneRunFrozenSnapshot(
                    original: run,
                    idea: idea,
                    compiler: compiler
                )
                // Start the cloned run
                let (plan, workspace) = try compiler.rebuildPlanFromSnapshot(run: clone)
                executionService.startRun(run: clone, plan: plan, workspace: workspace)
                dismiss()

            case .cloneRunCurrentConfig:
                guard let idea = run.idea else {
                    errorMessage = "No idea associated with this run"
                    break
                }
                guard let catalog = executionService.catalog else {
                    errorMessage = "No agent catalog available"
                    break
                }
                // Load workflow from source path
                guard let workflow = try? YAMLParser.loadWorkflow(
                    from: URL(fileURLWithPath: run.workflowSourcePath)
                ) else {
                    errorMessage = "Cannot load workflow from \(run.workflowSourcePath)"
                    break
                }
                let compiler = RunPlanCompiler(modelContext: modelContext)
                let clone = try coordinator.cloneRunCurrentConfig(
                    original: run,
                    idea: idea,
                    workflow: workflow,
                    catalog: catalog,
                    compiler: compiler,
                    workflowSourcePath: run.workflowSourcePath,
                    catalogSourcePath: run.catalogSourcePath
                )
                let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
                let (_, workspace) = try compiler.createRun(
                    for: idea,
                    plan: plan,
                    workflowSourcePath: run.workflowSourcePath,
                    catalogSourcePath: run.catalogSourcePath
                )
                executionService.startRun(run: clone, plan: plan, workspace: workspace)
                dismiss()
            }
        } catch {
            errorMessage = error.localizedDescription
        }

        isExecuting = false
    }

    // MARK: - Proposal 008 (§7.2): Stage Status Helpers

    // Proposal 013 §7.2: Action explanation with reuse/re-execution/same-run-vs-clone semantics
    private func recoveryActionExplanation(_ action: RecoveryAction) -> String {
        switch action {
        case .resumeRun(let stageID):
            return "Re-attaches the existing run and continues '\(stageID)' using the already prepared pending work."
        case .retryAgent(_, let agentID):
            return "Retries only agent '\(agentID)' in the same run. Successful sibling outputs are reused."
        case .retryAggregateStep(let stageID):
            return "Retries only the aggregate review step in '\(stageID)'. Contract-valid reviewer outputs are reused."
        case .retryStage(let stageID):
            return "Re-executes the entire '\(stageID)' stage in the same run. All agents re-run."
        case .resumeFromApprovalGate:
            return "Resumes from the approval gate in the same run. No re-execution."
        case .cloneRunFrozenSnapshot:
            return "Creates a new run using the frozen snapshot. This run becomes terminal history."
        case .cloneRunCurrentConfig:
            return "Creates a new run with the latest config. This run becomes terminal history."
        }
    }

    private func stageStatusIcon(_ status: StageStatus) -> String {
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

    private func stageStatusColor(_ status: StageStatus) -> Color {
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

    private var canonicalStageHistory: [StageExecution] {
        let grouped = Dictionary(grouping: run.stageExecutions) { stage in
            stage.lineageID ?? "\(stage.stageID)::\(stage.iteration)"
        }

        return grouped.values.compactMap { stages in
            stages.max { lhs, rhs in
                if lhs.attemptNumber != rhs.attemptNumber {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
        }
        .sorted { $0.startedAt < $1.startedAt }
    }
}
