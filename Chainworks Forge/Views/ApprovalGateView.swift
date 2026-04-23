import SwiftUI
import AppKit

// MARK: - ApprovalGateView (P031 diagnostic-only approval surface)

/// Displays a pending approval request as diagnostic-only readback.
struct ApprovalGateView: View {
    let request: ApprovalRequest

    private var diagnosticItems: [(label: String, value: String)] {
        [
            ("approval_id", request.id.uuidString),
            ("run_id", request.runID.uuidString),
            ("stage_id", request.stageID)
        ]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
            HStack(alignment: .top, spacing: ForgeSpacing.medium) {
                ForgeSectionHeader(
                    title: "Approval write path unavailable",
                    subtitle: request.stageLabel,
                    symbol: "lock.doc"
                )
                Spacer()
                Text(request.requestedAt, format: .dateTime)
                    .font(ForgeTypography.micro)
                    .foregroundStyle(ForgeColor.Text.tertiary)
            }

            Divider()

            VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                Text("Managed outside UI")
                    .font(ForgeTypography.supporting.weight(.semibold))
                    .foregroundStyle(DesignTokens.Action.caution)
                Text("Approval decisions are read-only in this operator surface. Use the approved external workflow or reference P031-FOLLOWUP-APPROVAL-WRITE-PATH.")
                    .font(ForgeTypography.supporting)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .accessibilityIdentifier("approval-diagnostic-callout")

            VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                LabeledContent("Stage", value: request.stageID)
                    .font(ForgeTypography.supporting)
                LabeledContent("Run", value: request.runID.uuidString)
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
                Text("Copy diagnostic identifiers")
                    .font(ForgeTypography.supporting)
                    .foregroundStyle(ForgeColor.Text.secondary)
                ForEach(diagnosticItems, id: \.label) { item in
                    HStack(spacing: ForgeSpacing.compact) {
                        LabeledContent(item.label, value: item.value)
                            .font(ForgeTypography.supporting)
                        Spacer(minLength: ForgeSpacing.compact)
                        Button {
                            copyDiagnosticValue(item.value)
                        } label: {
                            Label("Copy", systemImage: "doc.on.doc")
                                .labelStyle(.iconOnly)
                        }
                        .buttonStyle(.borderless)
                        .help("Copy \(item.label)")
                        .accessibilityLabel("Copy \(item.label)")
                    }
                }
            }
        }
        .forgePanel(tint: DesignTokens.Action.caution, fill: ForgeColor.Surface.elevated)
        .accessibilityIdentifier("approval-gate-view")
    }

    private func copyDiagnosticValue(_ value: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
    }
}

// MARK: - ApprovalInboxView (standalone list of all pending approvals)

/// Displays pending approval requests supplied by the GraphQL read surface.
struct ApprovalInboxView: View {
    let approvalRequests: [ApprovalRequest]

    private var sortedApprovals: [ApprovalRequest] {
        approvalRequests.sorted { $0.requestedAt < $1.requestedAt }
    }

    init(approvalRequests: [ApprovalRequest] = []) {
        self.approvalRequests = approvalRequests
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
