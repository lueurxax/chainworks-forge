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
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                // Header
                HStack {
                    Image(systemName: "checkmark.seal.fill")
                        .font(.title2)
                        .foregroundStyle(.orange)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Approval Required")
                            .font(.headline)
                        Text(request.stageLabel)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text(request.requestedAt, format: .dateTime)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }

                Divider()

                // Stage info
                VStack(alignment: .leading, spacing: 4) {
                    LabeledContent("Stage", value: request.stageID)
                        .font(.caption)
                    LabeledContent("Run", value: request.runID.uuidString.prefix(8) + "...")
                        .font(.caption)
                }

                // Preceding artifacts (§8.2, §11.4)
                if !request.precedingArtifacts.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Key artifacts for review:")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        ForEach(request.precedingArtifacts, id: \.self) { name in
                            HStack(spacing: 4) {
                                Image(systemName: "doc.text")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                Text(name)
                                    .font(.caption)
                                    .lineLimit(1)
                            }
                        }
                    }
                }

                // Comment field
                VStack(alignment: .leading, spacing: 4) {
                    Text("Comment (optional)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    TextField("Add a comment...", text: $comment, axis: .vertical)
                        .textFieldStyle(.roundedBorder)
                        .lineLimit(2...4)
                }

                // Action buttons
                HStack {
                    Button(role: .destructive) {
                        resolveApproval(granted: false)
                    } label: {
                        Label("Reject", systemImage: "xmark.circle")
                    }
                    .disabled(isResolving)
                    // Proposal 012 (L-09): Keyboard shortcut for reject
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
                    // Proposal 012 (L-09): Keyboard shortcut for approve
                    .keyboardShortcut(.return, modifiers: [.command])
                    .accessibilityIdentifier("approval-approve-button")
                }
            }
            .padding(4)
        }
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
