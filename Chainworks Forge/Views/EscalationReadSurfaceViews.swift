import AppKit
import Foundation
import SwiftUI

enum EscalationDensity: String, Sendable {
    case compact
    case standard
    case detailed
}

enum EscalationPresentationStyle {
    nonisolated static let statusLabelLimit = 24
    nonisolated static let commandTitleLimit = 48

    nonisolated static func stateLabel(for snapshot: EscalationSnapshot) -> String {
        if snapshot.isKillSwitchEngaged { return "Kill switch" }
        if snapshot.isPolicyDrift { return "Policy drift" }
        if snapshot.pausedChainCount > 0 { return "Paused" }
        if snapshot.hasActiveEscalation { return "Escalating" }
        return "Standard"
    }

    nonisolated static func stateSymbol(for snapshot: EscalationSnapshot) -> String {
        if snapshot.isKillSwitchEngaged { return "pause.circle.fill" }
        if snapshot.isPolicyDrift { return "exclamationmark.triangle.fill" }
        if snapshot.pausedChainCount > 0 { return "clock.badge.exclamationmark" }
        if snapshot.hasActiveEscalation { return "arrow.triangle.2.circlepath" }
        return "circle"
    }

    static func accentColor(for snapshot: EscalationSnapshot) -> Color {
        if let first = snapshot.activeChains.first,
           first.currentTierKindRaw == EscalationTierKindCode.sameBackendRetry.rawValue {
            if first.statusRaw == "paused" || first.statusRaw == "exhausted" {
                return .orange
            }
            if first.statusRaw == "active" || first.triggerRaw != nil {
                return .accentColor
            }
        }
        if snapshot.isKillSwitchEngaged || snapshot.isPolicyDrift { return .orange }
        if snapshot.pausedChainCount > 0 { return .yellow }
        if snapshot.hasActiveEscalation { return .accentColor }
        return .secondary
    }

    nonisolated static func tierLabel(for chain: EscalationChainStateDTO) -> String {
        chain.currentTierId ?? "Tier 0"
    }

    nonisolated static func triggerLabel(for chain: EscalationChainStateDTO) -> String {
        chain.triggerRaw ?? "Standard Execution"
    }

    nonisolated static func middleTruncated(_ value: String, limit: Int) -> String {
        guard limit > 3, value.count > limit else { return value }
        let available = limit - 1
        let prefixCount = max(1, available / 2)
        let suffixCount = max(1, available - prefixCount)
        return "\(value.prefix(prefixCount))...\(value.suffix(suffixCount))"
    }

    nonisolated static func countdownLabel(until rawDate: String?, now: Date = Date()) -> String? {
        guard let rawDate, let target = parseDate(rawDate) else { return nil }
        let seconds = max(0, Int(target.timeIntervalSince(now).rounded(.down)))
        if seconds == 0 { return "Ready to retry" }
        if seconds < 60 { return "\(seconds)s" }
        if seconds < 3_600 {
            return "\(seconds / 60):\(String(format: "%02d", seconds % 60))"
        }
        if seconds < 86_400 {
            return "\(seconds / 3_600):\(String(format: "%02d", (seconds % 3_600) / 60)):\(String(format: "%02d", seconds % 60))"
        }
        return "\(seconds / 86_400)d \((seconds % 86_400) / 3_600)h"
    }

    nonisolated static func durationLabel(createdAt: String, updatedAt: String) -> String? {
        guard let created = parseDate(createdAt), let updated = parseDate(updatedAt) else { return nil }
        let seconds = max(0, Int(updated.timeIntervalSince(created).rounded()))
        if seconds < 60 { return "\(seconds)s" }
        if seconds < 3_600 { return "\(seconds / 60)m \(seconds % 60)s" }
        return "\(seconds / 3_600)h \((seconds % 3_600) / 60)m"
    }

    nonisolated private static func parseDate(_ raw: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: raw) {
            return date
        }
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.date(from: raw)
    }

    nonisolated static func isShadowLineageRow(_ chain: EscalationChainStateDTO) -> Bool {
        chain.statusRaw == "shadow_only"
            || chain.statusRaw == "shadow"
            || chain.featureFlagState == "shadow"
            || chain.triggerRaw?.contains("shadow") == true
            || chain.pauseReasonRaw?.contains("shadow") == true
    }

    nonisolated static func pauseTitle(for reason: String?) -> String {
        switch reason {
        case EscalationPauseReasonCode.escalationKillSwitchEngaged.rawValue:
            return "Escalation kill switch engaged"
        case EscalationPauseReasonCode.escalationPolicyDrift.rawValue:
            return "Escalation policy drift"
        case EscalationPauseReasonCode.capacityProbeFailed.rawValue:
            return "Capacity probe failed"
        case EscalationPauseReasonCode.providerSessionForceDetached.rawValue:
            return "Provider session force detached"
        case EscalationPauseReasonCode.escalationRecoveryInconsistent.rawValue:
            return "Escalation recovery inconsistent"
        case EscalationPauseReasonCode.escalationChainExhausted.rawValue:
            return "Escalation chain exhausted"
        case .some(let value):
            return value.replacingOccurrences(of: "_", with: " ")
        case .none:
            return "Escalation paused"
        }
    }

    nonisolated static func accessibilitySummary(for snapshot: EscalationSnapshot) -> String {
        let state = stateLabel(for: snapshot)
        guard let first = snapshot.activeChains.first else {
            return "\(state), no escalation chain"
        }
        return [
            state,
            "tier \(tierLabel(for: first))",
            "trigger \(triggerLabel(for: first))",
            "policy \(first.policyId)",
            "ledger \(first.id)",
        ].joined(separator: ", ")
    }
}

enum EscalationSymbolCatalog {
    nonisolated static let requiredSymbolNames: [String] = [
        "pause.circle.fill",
        "exclamationmark.triangle.fill",
        "clock.badge.exclamationmark",
        "arrow.triangle.2.circlepath",
        "switch.2",
        "person.2.badge.gearshape",
        "pause.circle",
        "eye",
        "terminal",
    ]
}

struct EscalationStatusCapsulePresentation: Equatable {
    let stateLabel: String
    let tierLabel: String?
    let triggerLabel: String?
    let accessibilityLabel: String
    let helpText: String

    static func presentation(
        for snapshot: EscalationSnapshot,
        density: EscalationDensity
    ) -> EscalationStatusCapsulePresentation {
        let state = EscalationPresentationStyle.stateLabel(for: snapshot)
        guard let first = snapshot.activeChains.first else {
            return EscalationStatusCapsulePresentation(
                stateLabel: state,
                tierLabel: nil,
                triggerLabel: nil,
                accessibilityLabel: EscalationPresentationStyle.accessibilitySummary(for: snapshot),
                helpText: state
            )
        }

        let tier = EscalationPresentationStyle.middleTruncated(
            EscalationPresentationStyle.tierLabel(for: first),
            limit: EscalationPresentationStyle.statusLabelLimit
        )
        let trigger = EscalationPresentationStyle.middleTruncated(
            EscalationPresentationStyle.triggerLabel(for: first),
            limit: EscalationPresentationStyle.statusLabelLimit
        )
        let fullParts = [
            state,
            EscalationPresentationStyle.tierLabel(for: first),
            EscalationPresentationStyle.triggerLabel(for: first),
            first.policyId,
            first.id,
        ]
        return EscalationStatusCapsulePresentation(
            stateLabel: state,
            tierLabel: density == .compact ? nil : tier,
            triggerLabel: density == .detailed ? trigger : nil,
            accessibilityLabel: fullParts.joined(separator: ", "),
            helpText: fullParts.joined(separator: " • ")
        )
    }
}

struct EscalationStatusCapsule: View {
    let snapshot: EscalationSnapshot
    let density: EscalationDensity

    var body: some View {
        let presentation = EscalationStatusCapsulePresentation.presentation(for: snapshot, density: density)
        HStack(spacing: 5) {
            Label(presentation.stateLabel, systemImage: EscalationPresentationStyle.stateSymbol(for: snapshot))
                .padding(.horizontal, 7)
                .padding(.vertical, 3)
                .background(EscalationPresentationStyle.accentColor(for: snapshot).opacity(0.14), in: Capsule())
            if let tierLabel = presentation.tierLabel {
                Text("•")
                    .foregroundStyle(.secondary)
                Text(tierLabel)
                    .lineLimit(1)
            }
            if let triggerLabel = presentation.triggerLabel {
                Text("•")
                    .foregroundStyle(.secondary)
                Text(triggerLabel)
                    .lineLimit(1)
            }
        }
            .font(.caption.weight(.semibold))
            .lineLimit(1)
            .padding(.horizontal, density == .compact ? 0 : 2)
            .padding(.vertical, 2)
            .foregroundStyle(EscalationPresentationStyle.accentColor(for: snapshot))
            .help(presentation.helpText)
            .accessibilityLabel(presentation.accessibilityLabel)
            .accessibilityIdentifier("p058-escalation-status-capsule-\(density.rawValue)")
    }
}

struct EscalationBannerStack: View {
    let snapshot: EscalationSnapshot
    var density: EscalationDensity = .standard
    var onReviewDrift: (() -> Void)?
    var onOpenRunbook: ((String) -> Void)?

    var body: some View {
        let presentation = EscalationBannerStackPresentation.presentation(
            for: snapshot,
            density: density,
            onReviewDrift: onReviewDrift,
            onOpenRunbook: onOpenRunbook
        )
        VStack(alignment: .leading, spacing: 8) {
            ForEach(presentation.visibleBanners, id: \.id) { banner in
                HStack(spacing: 10) {
                    Image(systemName: banner.symbol)
                        .foregroundStyle(banner.color)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(banner.title)
                            .font(.subheadline.weight(.semibold))
                        Text(banner.subtitle)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    if let actionTitle = banner.actionTitle {
                        Button(actionTitle) {
                            banner.action()
                        }
                        .buttonStyle(.bordered)
                    }
                    if presentation.suppressedCount > 0, banner.id == presentation.visibleBanners.last?.id {
                        Text("+\(presentation.suppressedCount)")
                            .font(.caption2.weight(.semibold))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 3)
                            .background(.secondary.opacity(0.14), in: Capsule())
                            .help(presentation.suppressedTooltip)
                            .accessibilityLabel("\(presentation.suppressedCount) additional escalation banners: \(presentation.suppressedTooltip)")
                    }
                }
                .padding(10)
                .background(banner.color.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                .accessibilityElement(children: .combine)
            }
        }
        .accessibilityIdentifier("p058-escalation-banner-stack")
    }
}

struct EscalationBannerStackPresentation {
    let visibleBanners: [EscalationBanner]
    let suppressedCount: Int
    let suppressedTooltip: String

    static func presentation(
        for snapshot: EscalationSnapshot,
        density: EscalationDensity,
        onReviewDrift: (() -> Void)? = nil,
        onOpenRunbook: ((String) -> Void)? = nil
    ) -> EscalationBannerStackPresentation {
        let all = banners(for: snapshot, onReviewDrift: onReviewDrift, onOpenRunbook: onOpenRunbook)
        let visible = density == .compact ? Array(all.prefix(1)) : all
        let suppressed = density == .compact ? Array(all.dropFirst()) : []
        return EscalationBannerStackPresentation(
            visibleBanners: visible,
            suppressedCount: suppressed.count,
            suppressedTooltip: suppressed.map(\.title).joined(separator: ", ")
        )
    }

    private static func banners(
        for snapshot: EscalationSnapshot,
        onReviewDrift: (() -> Void)?,
        onOpenRunbook: ((String) -> Void)?
    ) -> [EscalationBanner] {
        var result: [EscalationBanner] = []
        if snapshot.isKillSwitchEngaged {
            result.append(EscalationBanner(
                id: "kill_switch",
                symbol: "pause.circle.fill",
                color: .orange,
                title: "Escalation disabled by kill switch",
                subtitle: "Existing ledger readback remains visible; scheduling stays on primary.",
                actionTitle: nil,
                action: {}
            ))
        }
        if snapshot.isPolicyDrift {
            result.append(EscalationBanner(
                id: "policy_drift",
                symbol: "exclamationmark.triangle.fill",
                color: .orange,
                title: "Escalation policy drift",
                subtitle: "Review frozen and current policy hashes through the external operator workflow.",
                actionTitle: "Review drift",
                action: { onReviewDrift?() }
            ))
        }
        if let chain = snapshot.activeChains.first(where: { $0.statusRaw == "paused" || $0.statusRaw == "exhausted" }) {
            result.append(EscalationBanner(
                id: "pause_\(chain.id)",
                symbol: "clock.badge.exclamationmark",
                color: .yellow,
                title: EscalationPresentationStyle.pauseTitle(for: chain.pauseReasonRaw),
                subtitle: chain.operatorActionHint ?? "Open the runbook before resuming or cancelling.",
                actionTitle: chain.runbookAnchor == nil ? nil : "Open runbook",
                action: {
                    if let runbook = chain.runbookAnchor {
                        onOpenRunbook?(runbook)
                    }
                }
            ))
        }
        return result
    }
}

struct EscalationBanner {
    let id: String
    let symbol: String
    let color: Color
    let title: String
    let subtitle: String
    let actionTitle: String?
    let action: () -> Void
}

struct EscalationLineageDisplayRow: Equatable, Identifiable {
    let id: String
    let chains: [EscalationChainStateDTO]
    let title: String
    let subtitle: String
    let detail: String?
    let attemptLabel: String
    let durationLabel: String?
    let expandedDetails: [String]
    let symbol: String
    let isShadow: Bool
    let isRetryCollapse: Bool
    var supportsDisclosure: Bool { !expandedDetails.isEmpty }

    static func rows(from chains: [EscalationChainStateDTO]) -> [EscalationLineageDisplayRow] {
        var result: [EscalationLineageDisplayRow] = []
        var index = chains.startIndex

        while index < chains.endIndex {
            let chain = chains[index]
            if chain.currentTierKindRaw == EscalationTierKindCode.sameBackendRetry.rawValue,
               let tierID = chain.currentTierId {
                var group: [EscalationChainStateDTO] = []
                var cursor = index
                while cursor < chains.endIndex,
                      chains[cursor].currentTierKindRaw == EscalationTierKindCode.sameBackendRetry.rawValue,
                      chains[cursor].currentTierId == tierID {
                    group.append(chains[cursor])
                    cursor = chains.index(after: cursor)
                }
                if group.count >= 3 {
                    let latest = group.last ?? chain
                    let maxAttempt = group.map(\.chainAttemptIndex).max() ?? group.count
                    result.append(EscalationLineageDisplayRow(
                        id: "retry-collapse-\(tierID)-\(group.first?.id ?? chain.id)-\(group.count)",
                        chains: group,
                        title: "Retry \(group.count) / \(maxAttempt)",
                        subtitle: "\(tierID) • \(EscalationPresentationStyle.triggerLabel(for: latest))",
                        detail: latest.pauseReasonRaw,
                        attemptLabel: "#\(latest.chainAttemptIndex)",
                        durationLabel: EscalationPresentationStyle.durationLabel(
                            createdAt: group.first?.createdAt ?? latest.createdAt,
                            updatedAt: latest.updatedAt
                        ),
                        expandedDetails: expandedDetails(for: latest),
                        symbol: "arrow.triangle.2.circlepath",
                        isShadow: group.contains(where: EscalationPresentationStyle.isShadowLineageRow),
                        isRetryCollapse: true
                    ))
                    index = cursor
                    continue
                }
            }

            result.append(EscalationLineageDisplayRow(
                id: chain.id,
                chains: [chain],
                title: EscalationPresentationStyle.tierLabel(for: chain),
                subtitle: "\(chain.statusRaw) • \(EscalationPresentationStyle.triggerLabel(for: chain))",
                detail: chain.pauseReasonRaw,
                attemptLabel: "#\(chain.chainAttemptIndex)",
                durationLabel: EscalationPresentationStyle.durationLabel(createdAt: chain.createdAt, updatedAt: chain.updatedAt),
                expandedDetails: expandedDetails(for: chain),
                symbol: symbol(for: chain),
                isShadow: EscalationPresentationStyle.isShadowLineageRow(chain),
                isRetryCollapse: false
            ))
            index = chains.index(after: index)
        }

        return result
    }

    private static func expandedDetails(for chain: EscalationChainStateDTO) -> [String] {
        [
            "ledger \(chain.id)",
            "policy \(chain.policyId)",
            "stage \(chain.stageId)",
            "agent \(chain.agentId)",
            "redaction v1",
            "trace \(chain.traceUnavailableReasonRaw ?? "available")",
        ]
    }

    private static func symbol(for chain: EscalationChainStateDTO) -> String {
        switch chain.currentTierKindRaw {
        case EscalationTierKindCode.sameBackendRetry.rawValue:
            return "arrow.triangle.2.circlepath"
        case EscalationTierKindCode.backendProfile.rawValue:
            return "switch.2"
        case EscalationTierKindCode.leadMediation.rawValue:
            return "person.2.badge.gearshape"
        case EscalationTierKindCode.pause.rawValue:
            return "pause.circle"
        default:
            return "circle"
        }
    }
}

struct EscalationLineageView: View {
    let snapshot: EscalationSnapshot
    @State private var expandedRows: Set<String> = []

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Escalation lineage")
                .font(.headline)
            if snapshot.activeChains.isEmpty {
                Text("No escalation policy activity")
                    .foregroundStyle(.secondary)
            } else {
                ForEach(EscalationLineageDisplayRow.rows(from: snapshot.activeChains)) { row in
                    EscalationLineageRowView(
                        row: row,
                        isExpanded: Binding(
                            get: { expandedRows.contains(row.id) },
                            set: { isExpanded in
                                if isExpanded {
                                    expandedRows.insert(row.id)
                                } else {
                                    expandedRows.remove(row.id)
                                }
                            }
                        )
                    )
                }
            }
        }
        .accessibilityIdentifier("p058-escalation-lineage-view")
    }
}

private struct EscalationLineageRowView: View {
    let row: EscalationLineageDisplayRow
    @Binding var isExpanded: Bool

    var body: some View {
        Group {
            if row.supportsDisclosure {
                DisclosureGroup(isExpanded: $isExpanded) {
                    VStack(alignment: .leading, spacing: 6) {
                        if row.isRetryCollapse {
                            ForEach(row.chains, id: \.id) { chain in
                                Text("\(chain.currentTierId ?? "retry") • \(chain.statusRaw) • \(EscalationPresentationStyle.triggerLabel(for: chain))")
                                    .font(.caption.monospaced())
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                            }
                        }
                        ForEach(row.expandedDetails, id: \.self) { detail in
                            Text(detail)
                                .font(.caption2.monospaced())
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                    }
                    .padding(.top, 4)
                } label: {
                    rowBody
                }
            } else {
                rowBody
            }
        }
        .focusable(true)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityIdentifier(row.isRetryCollapse ? "p058-escalation-lineage-retry-collapse" : "p058-escalation-lineage-row")
        .padding(8)
        .background(.secondary.opacity(row.isShadow ? 0.045 : 0.08), in: RoundedRectangle(cornerRadius: 8))
        .overlay(alignment: .leading) {
            if row.isShadow {
                Rectangle()
                    .stroke(style: StrokeStyle(lineWidth: 1, dash: [4, 4]))
                    .foregroundStyle(.secondary)
                    .frame(width: 1)
                    .padding(.vertical, 6)
            }
        }
        .opacity(row.isShadow ? 0.5 : 1)
    }

    private var rowBody: some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 10) {
                symbol
                titleBlock
                Spacer()
                trailingFacts
            }
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    symbol
                    titleBlock
                }
                trailingFacts
            }
        }
    }

    private var symbol: some View {
        Image(systemName: row.isShadow ? "eye" : row.symbol)
            .foregroundStyle(color)
            .frame(width: 18)
    }

    private var titleBlock: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(row.title)
                .font(.subheadline.weight(.semibold))
                .lineLimit(2)
            Text(row.subtitle)
                .font(row.isShadow ? .caption.italic() : .caption)
                .foregroundStyle(.secondary)
                .lineLimit(2)
            if let detail = row.detail {
                Text(detail)
                    .font(.caption2.monospaced())
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
        }
    }

    private var trailingFacts: some View {
        VStack(alignment: .trailing, spacing: 3) {
            Text(row.attemptLabel)
            if let durationLabel = row.durationLabel {
                Text(durationLabel)
            }
        }
        .font(.caption.monospacedDigit())
        .foregroundStyle(.secondary)
        .lineLimit(1)
    }

    private var color: Color {
        guard let first = row.chains.first else { return .secondary }
        if first.statusRaw == "paused" || first.statusRaw == "exhausted" {
            return .orange
        }
        if first.triggerRaw != nil {
            return .accentColor
        }
        return .secondary
    }

    private var accessibilityLabel: String {
        [
            row.isRetryCollapse ? "collapsed retry lineage" : "escalation lineage",
            row.title,
            row.subtitle,
            row.detail,
            row.attemptLabel,
            row.durationLabel,
            row.isShadow ? "shadow row" : nil,
        ]
        .compactMap { $0 }
        .joined(separator: ", ")
    }
}

struct EscalationPauseCardPresentation: Equatable {
    let title: String
    let body: String
    let countdownLabel: String?
    let metadata: String
    let accessibilityLabel: String

    static func presentation(for chain: EscalationChainStateDTO, now: Date = Date()) -> EscalationPauseCardPresentation {
        let title = EscalationPresentationStyle.pauseTitle(for: chain.pauseReasonRaw)
        let countdown = EscalationPresentationStyle.countdownLabel(until: chain.waitingRetryAfterUntil, now: now)
        let metadata = "Policy \(chain.policyId) • Tier \(chain.currentTierId ?? "Tier 0") • Trigger \(chain.triggerRaw ?? "none")"
        return EscalationPauseCardPresentation(
            title: title,
            body: chain.operatorActionHint ?? "Review the escalation runbook before taking action.",
            countdownLabel: countdown,
            metadata: metadata,
            accessibilityLabel: [title, countdown, metadata].compactMap { $0 }.joined(separator: ", ")
        )
    }

    func layout(forWidth width: CGFloat) -> EscalationPauseCardLayout {
        if width < 280 {
            return EscalationPauseCardLayout(
                mode: .oneLineSummary,
                summary: [title, countdownLabel].compactMap { $0 }.joined(separator: " • ")
            )
        }
        if width < 360 {
            return EscalationPauseCardLayout(mode: .minimumReadable, summary: title)
        }
        return EscalationPauseCardLayout(mode: .standard, summary: title)
    }
}

struct EscalationPauseCardLayout: Equatable {
    let mode: EscalationPauseCardLayoutMode
    let summary: String
}

enum EscalationPauseCardLayoutMode: Equatable {
    case oneLineSummary
    case minimumReadable
    case standard
}

struct EscalationPauseCard: View {
    let chain: EscalationChainStateDTO
    var onOpenRunbook: ((String) -> Void)?
    var onCopyDiagnostic: ((String) -> Void)?

    var body: some View {
        let presentation = EscalationPauseCardPresentation.presentation(for: chain)
        ViewThatFits(in: .horizontal) {
            pauseCardStandard(presentation)
            pauseCardMinimum(presentation)
            Label(
                presentation.layout(forWidth: 279).summary,
                systemImage: "pause.circle.fill"
            )
            .font(.caption.weight(.semibold))
            .lineLimit(1)
        }
        .padding(12)
        .frame(minWidth: 240, idealWidth: 480, minHeight: 44, alignment: .leading)
        .background(.orange.opacity(0.10), in: RoundedRectangle(cornerRadius: 8))
        .accessibilityElement(children: .combine)
        .accessibilityLabel(presentation.accessibilityLabel)
        .accessibilityIdentifier("p058-escalation-pause-card")
    }

    private func pauseCardStandard(_ presentation: EscalationPauseCardPresentation) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label(presentation.title, systemImage: "pause.circle.fill")
                    .font(.headline)
                Spacer()
                Text(presentation.countdownLabel ?? chain.statusRaw)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            Text(presentation.body)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            affordanceRow
            Text(presentation.metadata)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .lineLimit(2)
        }
    }

    private func pauseCardMinimum(_ presentation: EscalationPauseCardPresentation) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Label(presentation.title, systemImage: "pause.circle.fill")
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)
                Text(presentation.countdownLabel ?? chain.statusRaw)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            Text(presentation.metadata)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
    }

    private var affordanceRow: some View {
        HStack {
            if let runbook = chain.runbookAnchor {
                Button("Open runbook") { onOpenRunbook?(runbook) }
            }
            Button("Copy diagnostic bundle") { onCopyDiagnostic?(diagnosticBundle) }
        }
        .buttonStyle(.bordered)
    }

    private var diagnosticBundle: String {
        [
            "ledger_id": chain.id,
            "run_id": chain.runId,
            "stage_id": chain.stageId,
            "agent_id": chain.agentId,
            "policy_id": chain.policyId,
            "pause_reason": chain.pauseReasonRaw ?? "",
        ]
        .map { "\($0.key)=\($0.value)" }
        .joined(separator: "\n")
    }
}

struct EscalationCommandMirrorPresentation: Equatable {
    let title: String
    let fullTitle: String
    let subtitle: String
    let stateBadge: String?
    let disabledReason: String?
    let helpText: String
    let accessibilityLabel: String
    let accessibilityHint: String

    static func presentation(
        title: String,
        subtitle: String,
        stateBadge: String? = nil,
        disabledReason: String? = nil
    ) -> EscalationCommandMirrorPresentation {
        let displayedTitle = EscalationPresentationStyle.middleTruncated(
            title,
            limit: EscalationPresentationStyle.commandTitleLimit
        )
        let effectiveSubtitle = disabledReason ?? subtitle
        return EscalationCommandMirrorPresentation(
            title: displayedTitle,
            fullTitle: title,
            subtitle: effectiveSubtitle,
            stateBadge: stateBadge,
            disabledReason: disabledReason,
            helpText: [title, effectiveSubtitle, stateBadge].compactMap { $0 }.joined(separator: " • "),
            accessibilityLabel: [title, stateBadge, effectiveSubtitle].compactMap { $0 }.joined(separator: ", "),
            accessibilityHint: disabledReason ?? "Copies the mirrored operator command"
        )
    }
}

struct EscalationCommandMirrorRow: View {
    let title: String
    let subtitle: String
    let command: String
    var stateBadge: String?
    var disabledReason: String?
    var onCopyCommand: ((String) -> Void)?

    private var presentation: EscalationCommandMirrorPresentation {
        EscalationCommandMirrorPresentation.presentation(
            title: title,
            subtitle: subtitle,
            stateBadge: stateBadge,
            disabledReason: disabledReason
        )
    }

    private var subtitleColor: Color {
        disabledReason == nil ? .secondary : .orange
    }

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            commandIcon
            commandText
            Spacer()
            copyButton
        }
        .padding(10)
        .background(.secondary.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
        .help(presentation.helpText)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(presentation.accessibilityLabel)
        .accessibilityHint(presentation.accessibilityHint)
        .accessibilityIdentifier("p058-escalation-command-row")
    }

    private var commandIcon: some View {
        Image(systemName: "terminal")
            .foregroundStyle(.secondary)
            .frame(width: 18)
    }

    private var commandText: some View {
        VStack(alignment: .leading, spacing: 2) {
            commandTitleRow
            Text(presentation.subtitle)
                .font(.caption)
                .foregroundStyle(subtitleColor)
                .lineLimit(2)
        }
    }

    private var commandTitleRow: some View {
        HStack(spacing: 6) {
            Text(presentation.title)
                .font(.subheadline.weight(.semibold))
                .help(presentation.fullTitle)
            if let stateBadge = presentation.stateBadge {
                Text(stateBadge)
                    .font(.caption2.weight(.semibold))
                    .padding(.horizontal, 5)
                    .padding(.vertical, 2)
                    .background(.secondary.opacity(0.12), in: Capsule())
            }
        }
    }

    private var copyButton: some View {
        Button("Copy command") {
            onCopyCommand?(command)
        }
        .buttonStyle(.bordered)
        .disabled(disabledReason != nil)
        .accessibilityLabel("Copy escalation command for \(presentation.fullTitle)")
    }
}

struct EscalationCommandMirrorList: View {
    let snapshot: EscalationSnapshot
    var onCopyCommand: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Operator commands")
                .font(.headline)
            ForEach(commandRows, id: \.command) { row in
                EscalationCommandMirrorRow(
                    title: row.title,
                    subtitle: row.subtitle,
                    command: row.command,
                    stateBadge: row.stateBadge,
                    disabledReason: row.disabledReason,
                    onCopyCommand: onCopyCommand
                )
            }
        }
        .accessibilityIdentifier("p058-escalation-command-list")
    }

    private var commandRows: [(title: String, subtitle: String, command: String, stateBadge: String?, disabledReason: String?)] {
        snapshot.activeChains.compactMap { chain in
            guard let reason = chain.pauseReasonRaw else { return nil }
            let title = EscalationPresentationStyle.pauseTitle(for: reason)
            let command = [
                "reports.get",
                "run_id=\(chain.runId)",
                "stage_id=\(chain.stageId)",
                "ledger_id=\(chain.id)",
            ].joined(separator: " ")
            return (
                title: title,
                subtitle: chain.operatorActionHint ?? "Inspect the escalation readback before taking action.",
                command: command,
                stateBadge: chain.statusRaw,
                disabledReason: chain.statusRaw == "paused" ? nil : "Unavailable while escalation state is \(chain.statusRaw)."
            )
        }
    }
}

enum EscalationScreenState: String, CaseIterable, Sendable {
    case unavailable
    case subscribing
    case ready
    case stale
    case disconnected
    case decodeFailed
    case paused
    case drift
    case killSwitch
}

enum EscalationScreenStateMatrix {
    static func states(for snapshot: EscalationSnapshot) -> [EscalationScreenState] {
        var states: [EscalationScreenState] = []
        switch snapshot.readPipelineState {
        case .unavailable:
            states.append(.unavailable)
        case .subscribing:
            states.append(.subscribing)
        case .ready:
            states.append(.ready)
        case .stale:
            states.append(.stale)
        case .transportDisconnected:
            states.append(.disconnected)
        case .decodeFailed:
            states.append(.decodeFailed)
        }
        if snapshot.pausedChainCount > 0 {
            states.append(.paused)
        }
        if snapshot.isPolicyDrift {
            states.append(.drift)
        }
        if snapshot.isKillSwitchEngaged {
            states.append(.killSwitch)
        }
        return states
    }

    static func accessibilityLabel(for snapshot: EscalationSnapshot) -> String {
        states(for: snapshot)
            .map(\.rawValue)
            .joined(separator: ", ")
    }
}

struct EscalationMenuBarList: View {
    let snapshots: [EscalationSnapshot]
    var onOpenRun: ((String) -> Void)?
    var onShowAllPausedRuns: (() -> Void)?

    var body: some View {
        let presentation = EscalationMenuBarPresenter.presentation(for: snapshots)
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text("Escalation attention")
                    .font(.headline)
                Spacer()
                if presentation.aggregateCount > 0 {
                    Text("\(presentation.aggregateCount)")
                        .font(.caption.monospacedDigit().weight(.semibold))
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(.secondary.opacity(0.14), in: Capsule())
                        .accessibilityLabel("\(presentation.aggregateCount) runs require escalation attention")
                }
            }
            if presentation.rows.isEmpty {
                Text(presentation.emptyTitle)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(presentation.rows) { row in
                    Button {
                        onOpenRun?(row.runId)
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: row.symbol)
                                .foregroundStyle(row.accentColor)
                                .frame(width: 18)
                            VStack(alignment: .leading, spacing: 2) {
                                HStack(spacing: 6) {
                                    Text(row.runLabel)
                                        .font(.caption.weight(.semibold))
                                        .lineLimit(1)
                                    Text(row.stateLabel)
                                        .font(.caption2.weight(.semibold))
                                        .foregroundStyle(row.accentColor)
                                        .padding(.horizontal, 5)
                                        .padding(.vertical, 2)
                                        .background(row.accentColor.opacity(0.12), in: Capsule())
                                }
                                Text(row.detailLabel)
                                    .font(.caption.weight(.semibold))
                                    .lineLimit(1)
                                Text(row.secondaryLabel)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(row.accessibilityLabel)
                }
                if presentation.overflowCount > 0 {
                    Button {
                        onShowAllPausedRuns?()
                    } label: {
                        HStack(spacing: 6) {
                            Text(presentation.overflowTitle)
                            Text("+\(presentation.overflowCount)")
                                .foregroundStyle(.secondary)
                        }
                        .font(.caption2.weight(.semibold))
                    }
                    .buttonStyle(.plain)
                    .disabled(onShowAllPausedRuns == nil)
                    .accessibilityLabel("\(presentation.overflowCount) additional escalation runs. Show all paused runs.")
                }
            }
        }
        .padding(10)
        .frame(minWidth: 260)
        .accessibilityIdentifier("p058-escalation-menubar-list")
    }
}

struct EscalationMenuBarPresentation: Equatable {
    let rows: [EscalationMenuBarRow]
    let overflowCount: Int
    let overflowRunIDs: [String]
    let aggregateCount: Int
    let emptyTitle: String
    let overflowTitle: String
}

struct EscalationMenuBarRow: Identifiable, Equatable {
    let id: String
    let runId: String
    let runLabel: String
    let stateLabel: String
    let detailLabel: String
    let secondaryLabel: String
    let symbol: String
    let accentRole: EscalationMenuBarAccentRole
    let accessibilityLabel: String

    var accentColor: Color {
        switch accentRole {
        case .orange:
            return .orange
        case .yellow:
            return .yellow
        case .accent:
            return .accentColor
        case .secondary:
            return .secondary
        }
    }
}

enum EscalationMenuBarAccentRole: String, Equatable, Sendable {
    case orange
    case yellow
    case accent
    case secondary
}

enum EscalationMenuBarPresenter {
    nonisolated static let rowLimit = 5

    nonisolated static func presentation(
        for snapshots: [EscalationSnapshot],
        limit: Int = rowLimit
    ) -> EscalationMenuBarPresentation {
        let visibleLimit = max(limit, 0)
        let attention = snapshots
            .filter(isAttentionSnapshot)
            .sorted(by: compare)
        let rows = attention
            .prefix(visibleLimit)
            .map(row)
        let overflow = attention.dropFirst(visibleLimit)
        return EscalationMenuBarPresentation(
            rows: rows,
            overflowCount: overflow.count,
            overflowRunIDs: overflow.map(\.runId),
            aggregateCount: attention.count,
            emptyTitle: "No paused escalation runs",
            overflowTitle: "Show all paused runs..."
        )
    }

    nonisolated static func isAttentionSnapshot(_ snapshot: EscalationSnapshot) -> Bool {
        snapshot.pausedChainCount > 0
            || snapshot.isPolicyDrift
            || snapshot.isKillSwitchEngaged
            || snapshot.hasActiveEscalation
    }

    nonisolated private static func compare(_ lhs: EscalationSnapshot, _ rhs: EscalationSnapshot) -> Bool {
        let left = latestUpdatedAt(lhs)
        let right = latestUpdatedAt(rhs)
        if left != right {
            return left > right
        }
        return lhs.runId < rhs.runId
    }

    nonisolated private static func latestUpdatedAt(_ snapshot: EscalationSnapshot) -> String {
        snapshot.activeChains.map(\.updatedAt).max() ?? ""
    }

    nonisolated private static func row(for snapshot: EscalationSnapshot) -> EscalationMenuBarRow {
        let first = snapshot.activeChains.first
        let runLabel = compactRunLabel(snapshot.runId)
        let stateLabel = EscalationPresentationStyle.stateLabel(for: snapshot)
        let tier = first.map(EscalationPresentationStyle.tierLabel(for:)) ?? "No tier"
        let trigger = first.map(EscalationPresentationStyle.triggerLabel(for:)) ?? "no trigger"
        let paused = snapshot.pausedChainCount > 0 ? "\(snapshot.pausedChainCount) paused" : "no pause"
        let updated = latestUpdatedAt(snapshot)
        let detail = [tier, trigger].joined(separator: " / ")
        let secondary = [paused, updated].filter { !$0.isEmpty }.joined(separator: " / ")
        return EscalationMenuBarRow(
            id: snapshot.runId,
            runId: snapshot.runId,
            runLabel: runLabel,
            stateLabel: stateLabel,
            detailLabel: detail,
            secondaryLabel: secondary,
            symbol: EscalationPresentationStyle.stateSymbol(for: snapshot),
            accentRole: accentRole(for: snapshot),
            accessibilityLabel: [
                "Open escalation run \(snapshot.runId)",
                stateLabel,
                tier,
                trigger,
                paused,
            ].joined(separator: ", ")
        )
    }

    nonisolated private static func compactRunLabel(_ runId: String) -> String {
        guard runId.count > 12 else { return runId }
        return "\(runId.prefix(8))..."
    }

    nonisolated private static func accentRole(for snapshot: EscalationSnapshot) -> EscalationMenuBarAccentRole {
        if snapshot.isKillSwitchEngaged || snapshot.isPolicyDrift { return .orange }
        if snapshot.pausedChainCount > 0 { return .yellow }
        if snapshot.hasActiveEscalation { return .accent }
        return .secondary
    }
}

struct EscalationTraceTimeline: View {
    let traceJSONRedacted: String?
    var initiallyExpanded: Bool = false
    @State private var isExpanded: Bool

    init(traceJSONRedacted: String?, initiallyExpanded: Bool = false) {
        self.traceJSONRedacted = traceJSONRedacted
        self.initiallyExpanded = initiallyExpanded
        _isExpanded = State(initialValue: initiallyExpanded)
    }

    var body: some View {
        DisclosureGroup("Trace", isExpanded: $isExpanded) {
            if let traceJSONRedacted, !traceJSONRedacted.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text(traceJSONRedacted)
                        .font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled)
                        .lineLimit(12)
                    Button("Copy escalation trace") {
                        EscalationTracePasteboardWriter.copy(redactedTraceJSON: traceJSONRedacted)
                    }
                }
            } else {
                Text("Trace is unavailable for this escalation state.")
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("p058-escalation-trace-timeline")
    }
}

enum EscalationTracePasteboardWriter {
    static func copy(redactedTraceJSON: String, pasteboard: NSPasteboard = .general) {
        let data = Data(redactedTraceJSON.utf8)
        pasteboard.clearContents()
        pasteboard.setString(redactedTraceJSON, forType: .string)
        pasteboard.setData(data, forType: NSPasteboard.PasteboardType("public.json"))
    }
}

struct DriftReviewPresentation: Equatable {
    let rows: [DriftReviewRow]
    let isEmpty: Bool
    let handoffDetails: String

    static func presentation(
        runID: String,
        frozenPolicyHash: String,
        currentPolicyHash: String,
        acknowledgementCommand: String,
        frozenTierIds: [String] = [],
        currentTierIds: [String] = [],
        frozenTriggers: [String] = [],
        currentTriggers: [String] = [],
        frozenMaxChainAttempts: Int? = nil,
        currentMaxChainAttempts: Int? = nil
    ) -> DriftReviewPresentation {
        var rows: [DriftReviewRow] = [
            DriftReviewRow(label: "Run", frozen: runID, current: runID, badge: "context"),
            DriftReviewRow(
                label: "Policy hash",
                frozen: frozenPolicyHash,
                current: currentPolicyHash,
                badge: frozenPolicyHash == currentPolicyHash ? "unchanged" : "changed"
            ),
        ]
        rows.append(contentsOf: diffRows(label: "Tier", frozen: frozenTierIds, current: currentTierIds))
        rows.append(contentsOf: diffRows(label: "Trigger", frozen: frozenTriggers, current: currentTriggers))
        if frozenMaxChainAttempts != currentMaxChainAttempts {
            rows.append(DriftReviewRow(
                label: "Max chain attempts",
                frozen: frozenMaxChainAttempts.map(String.init) ?? "unset",
                current: currentMaxChainAttempts.map(String.init) ?? "unset",
                badge: "changed"
            ))
        }

        let handoff = [
            "run_id=\(runID)",
            "frozen_policy_hash=\(frozenPolicyHash)",
            "current_policy_hash=\(currentPolicyHash)",
            "acknowledgement_command=\(acknowledgementCommand)",
        ].joined(separator: "\n")
        return DriftReviewPresentation(
            rows: rows,
            isEmpty: rows.count <= 2 && frozenPolicyHash == currentPolicyHash,
            handoffDetails: handoff
        )
    }

    private static func diffRows(label: String, frozen: [String], current: [String]) -> [DriftReviewRow] {
        let frozenSet = Set(frozen)
        let currentSet = Set(current)
        let removed = frozenSet.subtracting(currentSet).sorted().map {
            DriftReviewRow(label: label, frozen: $0, current: "missing", badge: "removed")
        }
        let added = currentSet.subtracting(frozenSet).sorted().map {
            DriftReviewRow(label: label, frozen: "missing", current: $0, badge: "added")
        }
        let retained = frozenSet.intersection(currentSet).sorted().map {
            DriftReviewRow(label: label, frozen: $0, current: $0, badge: "unchanged")
        }
        return removed + added + retained
    }
}

struct DriftReviewRow: Equatable, Identifiable {
    var id: String { "\(label)-\(frozen)-\(current)-\(badge)" }
    let label: String
    let frozen: String
    let current: String
    let badge: String
}

struct DriftReviewSheet: View {
    var runID: String = "unknown"
    let frozenPolicyHash: String
    let currentPolicyHash: String
    let acknowledgementCommand: String
    var frozenTierIds: [String] = []
    var currentTierIds: [String] = []
    var frozenTriggers: [String] = []
    var currentTriggers: [String] = []
    var frozenMaxChainAttempts: Int?
    var currentMaxChainAttempts: Int?
    var onClose: () -> Void
    var onCopyCommand: ((String) -> Void)?
    var onOpenExternalWorkflow: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Escalation policy drift")
                .font(.title3.weight(.semibold))
            driftContent
            Text("Acknowledgement is handled outside the macOS read surface.")
                .foregroundStyle(.secondary)
            Text(presentation.handoffDetails)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
            actionRow
        }
        .padding(20)
        .frame(minWidth: 520)
        .interactiveDismissDisabled()
        .accessibilityIdentifier("p058-drift-review-sheet")
    }

    var presentationForTesting: DriftReviewPresentation {
        presentation
    }

    private var presentation: DriftReviewPresentation {
        DriftReviewPresentation.presentation(
            runID: runID,
            frozenPolicyHash: frozenPolicyHash,
            currentPolicyHash: currentPolicyHash,
            acknowledgementCommand: acknowledgementCommand,
            frozenTierIds: frozenTierIds,
            currentTierIds: currentTierIds,
            frozenTriggers: frozenTriggers,
            currentTriggers: currentTriggers,
            frozenMaxChainAttempts: frozenMaxChainAttempts,
            currentMaxChainAttempts: currentMaxChainAttempts
        )
    }

    @ViewBuilder
    private var driftContent: some View {
        if presentation.isEmpty {
            Text("Drift cleared while this sheet was open.")
                .foregroundStyle(.secondary)
        } else {
            driftGrid
        }
    }

    private var driftGrid: some View {
        Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 8) {
            GridRow {
                Text("Field").foregroundStyle(.secondary)
                Text("Frozen").foregroundStyle(.secondary)
                Text("Current").foregroundStyle(.secondary)
                Text("Delta").foregroundStyle(.secondary)
            }
            ForEach(presentation.rows) { row in
                driftGridRow(row)
            }
        }
    }

    private func driftGridRow(_ row: DriftReviewRow) -> some View {
        GridRow {
            Text(row.label)
            Text(row.frozen)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
            Text(row.current)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
            Text(row.badge)
                .font(.caption.weight(.semibold))
                .padding(.horizontal, 5)
                .padding(.vertical, 2)
                .background(.secondary.opacity(0.12), in: Capsule())
        }
    }

    private var actionRow: some View {
        HStack {
            Button("Copy acknowledgement command") {
                onCopyCommand?(acknowledgementCommand)
            }
            Button("Open external workflow") {
                onOpenExternalWorkflow?()
            }
            Spacer()
            Button("Close", action: onClose)
                .keyboardShortcut(.cancelAction)
        }
    }
}

struct EscalationInspector: View {
    let snapshot: EscalationSnapshot
    let traceJSONRedacted: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            EscalationStatusCapsule(snapshot: snapshot, density: .detailed)
            EscalationBannerStack(snapshot: snapshot)
            EscalationLineageView(snapshot: snapshot)
            if let paused = snapshot.activeChains.first(where: { $0.statusRaw == "paused" || $0.statusRaw == "exhausted" }) {
                EscalationPauseCard(chain: paused)
            }
            EscalationCommandMirrorList(snapshot: snapshot)
            EscalationTraceTimeline(traceJSONRedacted: traceJSONRedacted)
        }
        .padding(16)
        .accessibilityLabel(EscalationScreenStateMatrix.accessibilityLabel(for: snapshot))
        .accessibilityIdentifier("p058-escalation-inspector")
    }
}

struct EscalationInspectorAdapterView: View {
    @StateObject private var adapter: EscalationReadAdapter
    private let chains: [EscalationChainStateDTO]
    private let traceJSONRedacted: String?

    init(
        runID: String,
        chains: [EscalationChainStateDTO],
        traceJSONRedacted: String?
    ) {
        _adapter = StateObject(
            wrappedValue: EscalationReadAdapterRegistry.shared.adapter(for: runID)
        )
        self.chains = chains
        self.traceJSONRedacted = traceJSONRedacted
    }

    var body: some View {
        EscalationInspector(snapshot: adapter.snapshot, traceJSONRedacted: traceJSONRedacted)
            .onAppear {
                EscalationReadAdapterRegistry.shared.retainAdapter(for: adapter.runId)
            }
            .onDisappear {
                EscalationReadAdapterRegistry.shared.releaseAdapter(for: adapter.runId)
            }
            .task(id: chains) {
                adapter.applyChains(chains)
            }
            .accessibilityIdentifier("p058-escalation-inspector-adapter")
    }
}
