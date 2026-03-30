import SwiftUI
import SwiftData

// MARK: - ApprovalGateView (Proposal 002 — Approval inbox/detail surface)

/// Displays a pending approval request with approve/reject actions.
/// Used within RunProgressView for inline approval and as a standalone detail.
struct ApprovalGateView: View {
    @Environment(ExecutionService.self) private var executionService
    let request: ApprovalRequest
    @State private var comment: String = ""
    @State private var isResolving = false

    var body: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
            HStack(alignment: .top, spacing: ForgeSpacing.medium) {
                ForgeSectionHeader(
                    title: "Approval Required",
                    subtitle: request.stageLabel,
                    symbol: "checkmark.seal.fill"
                )
                Spacer()
                Text(request.requestedAt, format: .dateTime)
                    .font(ForgeTypography.micro)
                    .foregroundStyle(ForgeColor.Text.tertiary)
            }

            Divider()

            VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                LabeledContent("Stage", value: request.stageID)
                    .font(ForgeTypography.supporting)
                LabeledContent("Run", value: request.runID.uuidString.prefix(8) + "...")
                    .font(ForgeTypography.supporting)
            }

            if !request.precedingArtifacts.isEmpty {
                VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                    Text("Key artifacts for review:")
                        .font(ForgeTypography.supporting)
                        .foregroundStyle(ForgeColor.Text.secondary)
                    ForEach(request.precedingArtifacts, id: \.self) { name in
                        HStack(spacing: ForgeSpacing.compact) {
                            Image(systemName: "doc.text")
                                .font(ForgeTypography.micro)
                                .foregroundStyle(ForgeColor.Text.secondary)
                            Text(name)
                                .font(ForgeTypography.supporting)
                                .lineLimit(1)
                        }
                    }
                }
            }

            VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                Text("Comment (optional)")
                    .font(ForgeTypography.supporting)
                    .foregroundStyle(ForgeColor.Text.secondary)
                TextField("Add a comment...", text: $comment, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(2...4)
            }

            HStack {
                Button(role: .destructive) {
                    resolveApproval(granted: false)
                } label: {
                    Label("Reject", systemImage: "xmark.circle")
                }
                .disabled(isResolving)
                .keyboardShortcut(.delete, modifiers: [.command])
                .accessibilityIdentifier("approval-reject-button")

                Spacer()

                Button {
                    resolveApproval(granted: true)
                } label: {
                    Label("Approve", systemImage: "checkmark.circle.fill")
                }
                .buttonStyle(.borderedProminent)
                .tint(DesignTokens.Action.approve)
                .disabled(isResolving)
                .keyboardShortcut(.return, modifiers: [.command])
                .accessibilityIdentifier("approval-approve-button")
            }
        }
        .forgePanel(tint: DesignTokens.Action.caution, fill: ForgeColor.Surface.elevated)
        .accessibilityIdentifier("approval-gate-view")
    }

    private func resolveApproval(granted: Bool) {
        isResolving = true
        executionService.resolveApproval(
            approvalID: request.id,
            granted: granted,
            comment: comment.isEmpty ? nil : comment
        )
    }
}

// MARK: - ApprovalInboxView (standalone list of all pending approvals)

/// Displays all pending approval requests across all active runs.
/// Can be used as a standalone tab or navigation destination.
struct ApprovalInboxView: View {
    @Environment(ExecutionService.self) private var executionService

    private var sortedApprovals: [ApprovalRequest] {
        executionService.pendingApprovals.values
            .sorted { $0.requestedAt < $1.requestedAt }
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                if sortedApprovals.isEmpty {
                    VStack(spacing: 8) {
                        Image(systemName: "checkmark.seal")
                            .font(.largeTitle)
                            .foregroundStyle(.secondary)
                        Text("No Pending Approvals")
                            .font(.headline)
                            .accessibilityIdentifier("approval-inbox-empty-title")
                        Text("All approval gates have been resolved.")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity, minHeight: 180)
                    .accessibilityElement(children: .contain)
                    .accessibilityIdentifier("approval-inbox-empty-state")
                } else {
                    ForEach(sortedApprovals) { request in
                        ApprovalGateView(request: request)
                    }
                }
            }
            .padding()
        }
        .navigationTitle("Approval Inbox")
        .accessibilityIdentifier("approval-inbox-view")
    }
}
