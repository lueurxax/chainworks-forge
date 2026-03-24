import SwiftUI

// MARK: - P005-OPS §7.4: Recovery Sheet

/// Surfaces blocked/failed run recovery with:
/// - reason, most recent stage, trust/provenance summary,
/// - suggested next safe action, list of allowed actions.
/// Only exposes actions allowed for the current run type.
/// No repo-write, release, or publish recovery (§7.3).
struct RecoverySheet: View {
    let run: Run
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss
    @State private var recoveryContext: RecoveryContext?
    @State private var isExecuting = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                if let context = recoveryContext {
                    // Reason
                    GroupBox("Blocked Reason") {
                        Text(context.reason)
                            .font(.body)
                    }

                    // Most recent stage
                    GroupBox("Most Recent Stage") {
                        Text(context.mostRecentStage)
                            .font(.headline)
                    }

                    // Trust / provenance summary
                    GroupBox("Runtime Trust") {
                        RuntimeProvenanceBadge(trustLevel: context.trustSummary)
                    }

                    Divider()

                    // Suggested action
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

                    // All allowed actions
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

                    // Error message
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
            case .retryStage(let stageID):
                _ = try coordinator.retryStage(run: run, stageID: stageID)
            case .resumeFromApprovalGate(let stageID):
                _ = try coordinator.resumeFromApprovalGate(run: run, stageID: stageID)
            case .cloneRunFrozenSnapshot:
                // Needs compiler — simplified for now
                errorMessage = "Clone requires RunPlanCompiler. Use from Run context."
            case .cloneRunCurrentConfig:
                errorMessage = "Clone requires workflow + catalog. Use from Run context."
            }
            if errorMessage == nil {
                dismiss()
            }
        } catch {
            errorMessage = error.localizedDescription
        }

        isExecuting = false
    }
}
