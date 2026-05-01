// P073 §I: Pre-quarantine acknowledgment banner.
//
// I1: One-time banner shown to the operator before the P073 mutation
//     quarantine takes effect. Uses AppStorage so it appears only once
//     per install and is not tied to a run or session.
//
// I2: Persistent Scheduler Health chip variant — a compact inline chip
//     shown inside the banner when a SchedulerHealthBannerIssue is active,
//     so late-arrivers (operators opening the app after freeze begins) still
//     see the scheduler health signal even if the main lifecycle banner is
//     in a quiet state.

import SwiftUI

// MARK: - I1: One-time acknowledgment banner

struct P073QuarantineAcknowledgmentBanner: View {
    @AppStorage("p073_quarantine_acknowledged") private var acknowledged = false
    var schedulerHealthIssue: SchedulerHealthBannerIssue?
    var onOpenSchedulerHealth: (() -> Void)?

    var body: some View {
        if !acknowledged {
            bannerBody
        }
    }

    private var bannerBody: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.small) {
            HStack(spacing: ForgeSpacing.small) {
                Image(systemName: "lock.shield.fill")
                    .foregroundStyle(ForgeStatusColor.warning)
                Text("Stabilization freeze active (P073)")
                    .font(.callout.weight(.semibold))
                Spacer(minLength: 0)
                Button {
                    withAnimation {
                        acknowledged = true
                    }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(ForgeColor.Text.tertiary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Dismiss stabilization freeze notice")
                .accessibilityIdentifier("p073-quarantine-dismiss")
            }
            Text(
                "Write operations from the app are quarantined during the P073 stability window. "
                + "Use MCP server commands to start runs, retry stages, or resolve approvals."
            )
            .font(.callout)
            .foregroundStyle(ForgeColor.Text.secondary)
            if let issue = schedulerHealthIssue {
                // I2: Late-arriver scheduler health chip
                SchedulerHealthChip(issue: issue, onOpen: onOpenSchedulerHealth)
            }
        }
        .padding(ForgeSpacing.medium)
        .forgePanel(tint: ForgeStatusColor.warning)
        .transition(.move(edge: .top).combined(with: .opacity))
        .accessibilityIdentifier("p073-quarantine-acknowledgment-banner")
    }
}

// MARK: - I2: Scheduler health chip (late-arriver surface)

struct SchedulerHealthChip: View {
    let issue: SchedulerHealthBannerIssue
    var onOpen: (() -> Void)?

    var body: some View {
        Button {
            onOpen?()
        } label: {
            HStack(spacing: ForgeSpacing.compact) {
                Image(systemName: issue.systemImage)
                    .font(.caption)
                Text(issue.title)
                    .font(.caption.weight(.medium))
                if onOpen != nil {
                    Image(systemName: "chevron.right")
                        .font(.caption2)
                        .foregroundStyle(ForgeColor.Text.tertiary)
                }
            }
            .foregroundStyle(schedulerTint(issue.kind))
            .padding(.horizontal, ForgeSpacing.small)
            .padding(.vertical, ForgeSpacing.compact)
            .background(
                schedulerTint(issue.kind).opacity(0.12),
                in: Capsule()
            )
        }
        .buttonStyle(.plain)
        .disabled(onOpen == nil)
        .accessibilityLabel("\(issue.title) — \(issue.detail)")
        .accessibilityIdentifier("p073-scheduler-health-chip")
    }

    private func schedulerTint(_ kind: SchedulerHealthBannerIssue.Kind) -> Color {
        switch kind {
        case .sustainedBackpressure, .dbWriterPressure: return ForgeStatusColor.warning
        case .staleProjection: return Color.yellow
        }
    }
}
