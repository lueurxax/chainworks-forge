// P073 §H: Blocked-action panel — shown when a GraphQL mutation is rejected
// because the app credential (forge-app-graphql / app_graphql_readonly) does
// not hold write rights during the stabilization freeze.
//
// H1: Panel structure — derives from the ForgePanel component family.
// H2: Copy Command affordance — operator can copy the equivalent MCP command.
// H3: Learn More deep link — opens the stability gate reference doc.

import SwiftUI

/// Describes a mutation that was blocked by the P073 mutation quarantine.
struct BlockedActionContext: Sendable {
    /// Human-readable name for the blocked operation (e.g. "Start Run").
    let actionName: String
    /// Canonical MCP command the operator can use instead (server-side truth).
    let canonicalMCPCommand: String
    /// URL string for operator docs (Learn More target).
    let learnMoreURL: String

    static func standard(for mutationName: String) -> BlockedActionContext {
        let humanName: String
        let mcpCommand: String
        switch mutationName {
        case "startRun":
            humanName = "Start Run"
            mcpCommand = #"runs.start --idea-id <IDEA_ID>"#
        case "retryStage":
            humanName = "Retry Stage"
            mcpCommand = #"stages.retry --stage-id <STAGE_ID>"#
        case "cancelRun":
            humanName = "Cancel Run"
            mcpCommand = #"runs.cancel --run-id <RUN_ID>"#
        case "approveApproval":
            humanName = "Approve"
            mcpCommand = #"approvals.resolve --approval-id <APPROVAL_ID> --decision approve"#
        default:
            humanName = mutationName
            mcpCommand = "# Use the MCP server to perform this action"
        }
        return BlockedActionContext(
            actionName: humanName,
            canonicalMCPCommand: mcpCommand,
            learnMoreURL: "https://github.com/chainworks/forge/docs/reference/p073-mutation-quarantine"
        )
    }
}

/// Panel displayed in-context when an operator action is blocked by the
/// P073 mutation quarantine. Provides copy-command affordance (H2) and
/// a Learn More link (H3).
struct BlockedActionPanel: View {
    let context: BlockedActionContext
    var onDismiss: (() -> Void)?

    @State private var copyConfirmed = false

    var body: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
            titleRow
            bodyText
            commandBlock
            actionRow
        }
        .padding(ForgeSpacing.large)
        .forgePanel(tint: ForgeStatusColor.warning)
        .accessibilityIdentifier("blocked-action-panel")
    }

    private var titleRow: some View {
        HStack(spacing: ForgeSpacing.small) {
            Image(systemName: "lock.trianglebadge.exclamationmark.fill")
                .foregroundStyle(ForgeStatusColor.warning)
            Text(""\(context.actionName)" is quarantined during stabilization")
                .font(.callout.weight(.semibold))
                .foregroundStyle(ForgeColor.Text.primary)
            Spacer(minLength: 0)
            if let onDismiss {
                Button {
                    onDismiss()
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(ForgeColor.Text.tertiary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Dismiss blocked action panel")
            }
        }
    }

    private var bodyText: some View {
        Text(
            "The app credential (forge-app-graphql) is read-only during the P073 stability "
            + "freeze. Use the MCP server to perform this action:"
        )
        .font(.callout)
        .foregroundStyle(ForgeColor.Text.secondary)
    }

    private var commandBlock: some View {
        HStack(spacing: ForgeSpacing.small) {
            Text(context.canonicalMCPCommand)
                .font(.callout.monospaced())
                .foregroundStyle(ForgeColor.Text.primary)
                .textSelection(.enabled)
                .lineLimit(3)
            Spacer(minLength: 0)
            Button {
                copyCommand()
            } label: {
                Label(
                    copyConfirmed ? "Copied" : "Copy",
                    systemImage: copyConfirmed ? "checkmark" : "doc.on.doc"
                )
            }
            .controlSize(.small)
            .accessibilityIdentifier("blocked-action-copy-command")
        }
        .padding(ForgeSpacing.small)
        .background(ForgeColor.Surface.muted, in: RoundedRectangle(cornerRadius: ForgeRadius.card))
    }

    private var actionRow: some View {
        HStack {
            Spacer(minLength: 0)
            if let url = URL(string: context.learnMoreURL) {
                Link("Learn More", destination: url)
                    .font(.callout)
                    .accessibilityIdentifier("blocked-action-learn-more")
            }
        }
    }

    private func copyCommand() {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(context.canonicalMCPCommand, forType: .string)
        #endif
        withAnimation {
            copyConfirmed = true
        }
        Task {
            try? await Task.sleep(for: .seconds(2))
            await MainActor.run {
                withAnimation { copyConfirmed = false }
            }
        }
    }
}
