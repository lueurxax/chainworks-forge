import SwiftUI

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
                        RuntimeProvenanceBadge(trustLevel: context.trustSummary)
                    }

                    Divider()

                    if let suggested = context.suggestedAction {
                        GroupBox("Suggested Action") {
                            HStack {
                                Image(systemName: suggested.systemImage)
                                    .font(.title3)
                                    .foregroundStyle(.blue)
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
                                    Text(action.label)
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
                            ForEach(run.stageExecutions.sorted(by: { $0.startedAt < $1.startedAt })) { stage in
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
                                            .foregroundStyle(.blue)
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
        }
    }

    private func executeAction(_ action: RecoveryAction) {
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
}
