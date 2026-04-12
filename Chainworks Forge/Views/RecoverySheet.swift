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

    private var stageSnapshots: [RunStageSnapshot] {
        RunStageSnapshotLoader.load(for: run, modelContext: modelContext)
    }

    private var preservedReceiptArtifacts: [Artifact] {
        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID
            },
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        return ((try? modelContext.fetch(descriptor)) ?? [])
            .filter { $0.name.contains("receipt") || $0.contractID == "provider_receipt" || $0.contractID == "delivery_receipt" }
    }

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

                    // Proposal 013 (§7.3): Evidence summary
                    if let evidenceSummary = context.evidenceSummary {
                        GroupBox("Evidence") {
                            HStack {
                                Image(systemName: "doc.text.magnifyingglass")
                                    .foregroundStyle(DesignTokens.Status.warning)
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
                                    .tint(DesignTokens.Action.primary)
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
                                    .foregroundStyle(actionColor(suggested))
                                VStack(alignment: .leading) {
                                    Text(suggested.label)
                                        .font(.headline)
                                    Text("Recommended next step")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Button("Execute") {
                                    beginExecuteAction(suggested)
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(actionColor(suggested))
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
                                        .foregroundStyle(actionColor(action))
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
                                        beginExecuteAction(action)
                                    }
                                    .buttonStyle(.bordered)
                                    .tint(actionColor(action))
                                    .disabled(isExecuting)
                                }
                            }
                        }
                    }

                    // Proposal 008 (§7.2): Stage history disclosure
                    DisclosureGroup("Stage History", isExpanded: $showStageHistory) {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(stageSnapshots) { stage in
                                HStack {
                                    Image(systemName: stageStatusIcon(stage.status))
                                        .foregroundStyle(stageStatusColor(stage.status))
                                        .frame(width: 20)
                                    Text(stage.label)
                                        .font(.caption)
                                    Spacer()
                                    StatusCapsule(
                                        text: stage.status.rawValue.replacingOccurrences(of: "_", with: " ").capitalized,
                                        color: stageStatusColor(stage.status),
                                        size: .small,
                                        accessibilityIdentifier: "recovery-stage-\(stage.id)-status"
                                    )
                                    if stage.attemptNumber > 1 {
                                        Text("(attempt \(stage.attemptNumber))")
                                            .font(.caption2)
                                            .foregroundStyle(DesignTokens.Status.warning)
                                    }
                                }
                            }
                        }
                    }

                    // Proposal 008 (§7.2): Preserved receipts disclosure
                    if !preservedReceiptArtifacts.isEmpty {
                        DisclosureGroup("Preserved Receipts (\(preservedReceiptArtifacts.count))", isExpanded: $showPreservedReceipts) {
                            VStack(alignment: .leading, spacing: 4) {
                                ForEach(preservedReceiptArtifacts) { receipt in
                                    HStack {
                                        Image(systemName: "doc.text.fill")
                                            .foregroundStyle(DesignTokens.Status.success)
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
                        .foregroundStyle(DesignTokens.Status.error)
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

    @MainActor
    private func beginExecuteAction(_ action: RecoveryAction) {
        guard !isExecuting else { return }
        isExecuting = true
        errorMessage = nil

        Task { @MainActor in
            await executeAction(action)
        }
    }

    @MainActor
    private func executeAction(_ action: RecoveryAction) async {
        let coordinator = RecoveryCoordinator(modelContext: modelContext)

        do {
            switch action {
            case .retryAgent(let stageID, let agentID):
                _ = try coordinator.retryAgent(run: run, stageID: stageID, agentID: agentID)
                let compiler = RunPlanCompiler(modelContext: modelContext)
                try executionService.resumeRun(run: run, compiler: compiler)
                dismiss()

            case .retryAggregateStep(let stageID, let agentID):
                _ = try coordinator.retryAggregateStep(run: run, stageID: stageID, agentID: agentID)
                let compiler = RunPlanCompiler(modelContext: modelContext)
                try executionService.resumeRun(run: run, compiler: compiler)
                dismiss()

            case .retryStage(let stageID):
                _ = try coordinator.retryStage(run: run, stageID: stageID)
                let compiler = RunPlanCompiler(modelContext: modelContext)
                try executionService.resumeRun(run: run, compiler: compiler)
                dismiss()

            case .resumeInterrupted:
                let compiler = RunPlanCompiler(modelContext: modelContext)
                try executionService.resumeRun(run: run, compiler: compiler)
                dismiss()

            case .resumeFromApprovalGate(let stageID):
                _ = try coordinator.resumeFromApprovalGate(run: run, stageID: stageID)
                dismiss()

            case .resetAgentSession(let stageID, let agentID):
                _ = try await coordinator.resetAgentSession(run: run, stageID: stageID, agentID: agentID)
                // Just reset, refresh context
                recoveryContext = coordinator.recoveryContext(for: run)

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
                let workflow: WorkflowDefinition
                do {
                    workflow = try YAMLParser.loadWorkflow(
                        from: URL(fileURLWithPath: run.workflowSourcePath)
                    )
                } catch {
                    ForgeLogger.ui.error(
                        "RecoverySheet failed to load workflow for run \(run.id) from \(run.workflowSourcePath): \(error.localizedDescription)"
                    )
                    errorMessage = "Cannot load workflow from \(run.workflowSourcePath): \(error.localizedDescription)"
                    break
                }
                let compiler = RunPlanCompiler(modelContext: modelContext)
                let strategySelection = StrategyExperimentCoordinator(config: executionService.stewardConfig)
                    .resolveSelection(
                        selectedProfileID: run.contextStrategyProfileID,
                        cohortID: run.experimentCohortID
                    )
                let startSnapshot = RunStartSnapshot(
                    providerBindingSnapshotJSON: nil,
                    bindingProvenanceJSON: nil,
                    startOptionsJSON: nil,
                    frozenWorkspaceRootPath: run.frozenWorkspaceRootPath,
                    deliveryConfiguration: nil,
                    deliveryPreflightJSON: nil,
                    contextStrategyProfileID: strategySelection.profileID,
                    strategyAssignmentMode: strategySelection.assignmentMode,
                    strategyRecommendationState: strategySelection.recommendationState,
                    contextStrategySnapshotJSON: try JSONEncoder().encode(strategySelection.profile),
                    promotedHandoffArtifactsJSON: run.promotedHandoffArtifactsJSON
                )
                let clone = try coordinator.cloneRunCurrentConfig(
                    original: run,
                    idea: idea,
                    workflow: workflow,
                    catalog: catalog,
                    compiler: compiler,
                    workflowSourcePath: run.workflowSourcePath,
                    catalogSourcePath: run.catalogSourcePath,
                    startSnapshot: startSnapshot
                )
                let plan = try compiler.previewCompile(
                    workflow: workflow,
                    catalog: catalog,
                    catalogSourcePath: run.catalogSourcePath
                )
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
        case .retryAgent(_, let agentID):
            return "Retries only agent '\(agentID)' in the same run. Successful sibling outputs are reused."
        case .retryAggregateStep(_, let agentID):
            return "Retries only the aggregate step '\(agentID)' in the same run. Reviewer outputs are reused."
        case .retryStage(let stageID):
            return "Re-executes the entire '\(stageID)' stage in the same run. All agents re-run."
        case .resumeInterrupted(let stageID):
            return "Continues the interrupted run from the prepared '\(stageID)' continuation point without rewriting stage lineage."
        case .resumeFromApprovalGate:
            return "Resumes from the approval gate in the same run. No re-execution."
        case .resetAgentSession(_, let agentID):
            return "Discard live provider session for '\(agentID)' and force a fresh session on retry."
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
        case .completed: return DesignTokens.Status.success
        case .failed: return DesignTokens.Status.error
        case .running: return DesignTokens.Status.running
        case .waitingApproval: return DesignTokens.Status.warning
        case .blocked: return DesignTokens.Status.error
        case .skipped: return DesignTokens.Status.cancelled
        case .pending, .ready: return DesignTokens.Status.neutral
        }
    }

    private func actionColor(_ action: RecoveryAction) -> Color {
        switch action {
        case .resumeInterrupted, .resumeFromApprovalGate: return DesignTokens.Action.approve
        case .retryAgent, .retryAggregateStep, .retryStage: return DesignTokens.Action.caution
        case .resetAgentSession: return DesignTokens.Status.error
        case .cloneRunFrozenSnapshot: return DesignTokens.Action.primary
        case .cloneRunCurrentConfig: return DesignTokens.Status.neutral
        }
    }
}
