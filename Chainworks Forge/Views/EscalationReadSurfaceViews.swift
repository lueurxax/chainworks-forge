import AppKit
import SwiftUI

enum EscalationDensity: String, Sendable {
    case compact
    case standard
    case detailed
}

enum EscalationPresentationStyle {
    static func stateLabel(for snapshot: EscalationSnapshot) -> String {
        if snapshot.isKillSwitchEngaged { return "Kill switch" }
        if snapshot.isPolicyDrift { return "Policy drift" }
        if snapshot.pausedChainCount > 0 { return "Paused" }
        if snapshot.hasActiveEscalation { return "Escalating" }
        return "Standard"
    }

    static func stateSymbol(for snapshot: EscalationSnapshot) -> String {
        if snapshot.isKillSwitchEngaged { return "pause.circle.fill" }
        if snapshot.isPolicyDrift { return "exclamationmark.triangle.fill" }
        if snapshot.pausedChainCount > 0 { return "clock.badge.exclamationmark" }
        if snapshot.hasActiveEscalation { return "arrow.triangle.2.circlepath" }
        return "circle"
    }

    static func accentColor(for snapshot: EscalationSnapshot) -> Color {
        if snapshot.isKillSwitchEngaged || snapshot.isPolicyDrift { return .orange }
        if snapshot.pausedChainCount > 0 { return .yellow }
        if snapshot.hasActiveEscalation { return .accentColor }
        return .secondary
    }

    static func tierLabel(for chain: EscalationChainStateDTO) -> String {
        chain.currentTierId ?? "Tier 0"
    }

    static func triggerLabel(for chain: EscalationChainStateDTO) -> String {
        chain.triggerRaw ?? "standard execution"
    }

    static func pauseTitle(for reason: String?) -> String {
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

    static func accessibilitySummary(for snapshot: EscalationSnapshot) -> String {
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

struct EscalationStatusCapsule: View {
    let snapshot: EscalationSnapshot
    let density: EscalationDensity

    var body: some View {
        let label = EscalationPresentationStyle.stateLabel(for: snapshot)
        Label(label, systemImage: EscalationPresentationStyle.stateSymbol(for: snapshot))
            .font(.caption.weight(.semibold))
            .lineLimit(1)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .foregroundStyle(EscalationPresentationStyle.accentColor(for: snapshot))
            .background(EscalationPresentationStyle.accentColor(for: snapshot).opacity(0.14), in: Capsule())
            .help(helpText)
            .accessibilityLabel(EscalationPresentationStyle.accessibilitySummary(for: snapshot))
            .accessibilityIdentifier("p058-escalation-status-capsule-\(density.rawValue)")
    }

    private var helpText: String {
        guard density != .compact, let first = snapshot.activeChains.first else {
            return EscalationPresentationStyle.stateLabel(for: snapshot)
        }
        return [
            EscalationPresentationStyle.stateLabel(for: snapshot),
            EscalationPresentationStyle.tierLabel(for: first),
            EscalationPresentationStyle.triggerLabel(for: first),
        ].joined(separator: " • ")
    }
}

struct EscalationBannerStack: View {
    let snapshot: EscalationSnapshot
    var onReviewDrift: (() -> Void)?
    var onOpenRunbook: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(banners, id: \.id) { banner in
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
                }
                .padding(10)
                .background(banner.color.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                .accessibilityElement(children: .combine)
            }
        }
        .accessibilityIdentifier("p058-escalation-banner-stack")
    }

    private var banners: [EscalationBanner] {
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

private struct EscalationBanner {
    let id: String
    let symbol: String
    let color: Color
    let title: String
    let subtitle: String
    let actionTitle: String?
    let action: () -> Void
}

struct EscalationLineageView: View {
    let snapshot: EscalationSnapshot

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Escalation lineage")
                .font(.headline)
            if snapshot.activeChains.isEmpty {
                Text("No escalation policy activity")
                    .foregroundStyle(.secondary)
            } else {
                ForEach(snapshot.activeChains, id: \.id) { chain in
                    HStack(alignment: .top, spacing: 10) {
                        Image(systemName: symbol(for: chain))
                            .foregroundStyle(color(for: chain))
                            .frame(width: 18)
                        VStack(alignment: .leading, spacing: 3) {
                            Text(EscalationPresentationStyle.tierLabel(for: chain))
                                .font(.subheadline.weight(.semibold))
                            Text("\(chain.statusRaw) • \(EscalationPresentationStyle.triggerLabel(for: chain))")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                            if let reason = chain.pauseReasonRaw {
                                Text(reason)
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        Text("#\(chain.chainAttemptIndex)")
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                    .padding(8)
                    .background(.secondary.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
                    .accessibilityElement(children: .combine)
                }
            }
        }
        .accessibilityIdentifier("p058-escalation-lineage-view")
    }

    private func symbol(for chain: EscalationChainStateDTO) -> String {
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

    private func color(for chain: EscalationChainStateDTO) -> Color {
        if chain.statusRaw == "paused" || chain.statusRaw == "exhausted" {
            return .orange
        }
        if chain.triggerRaw != nil {
            return .accentColor
        }
        return .secondary
    }
}

struct EscalationPauseCard: View {
    let chain: EscalationChainStateDTO
    var onOpenRunbook: ((String) -> Void)?
    var onCopyDiagnostic: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label(
                    EscalationPresentationStyle.pauseTitle(for: chain.pauseReasonRaw),
                    systemImage: "pause.circle.fill"
                )
                .font(.headline)
                Spacer()
                Text(chain.statusRaw)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            Text(chain.operatorActionHint ?? "Review the escalation runbook before taking action.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
            HStack {
                if let runbook = chain.runbookAnchor {
                    Button("Open runbook") { onOpenRunbook?(runbook) }
                }
                Button("Copy diagnostic bundle") { onCopyDiagnostic?(diagnosticBundle) }
            }
            .buttonStyle(.bordered)
            Text("Policy \(chain.policyId) • Tier \(chain.currentTierId ?? "Tier 0") • Trigger \(chain.triggerRaw ?? "none")")
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .lineLimit(2)
        }
        .padding(12)
        .background(.orange.opacity(0.10), in: RoundedRectangle(cornerRadius: 8))
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("p058-escalation-pause-card")
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

struct DriftReviewSheet: View {
    let frozenPolicyHash: String
    let currentPolicyHash: String
    let acknowledgementCommand: String
    var onClose: () -> Void
    var onCopyCommand: ((String) -> Void)?
    var onOpenExternalWorkflow: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Escalation policy drift")
                .font(.title3.weight(.semibold))
            Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 8) {
                GridRow {
                    Text("Frozen")
                        .foregroundStyle(.secondary)
                    Text(frozenPolicyHash)
                        .font(.system(.body, design: .monospaced))
                        .textSelection(.enabled)
                }
                GridRow {
                    Text("Current")
                        .foregroundStyle(.secondary)
                    Text(currentPolicyHash)
                        .font(.system(.body, design: .monospaced))
                        .textSelection(.enabled)
                }
            }
            Text("Acknowledgement is handled outside the macOS read surface.")
                .foregroundStyle(.secondary)
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
        .padding(20)
        .frame(minWidth: 520)
        .interactiveDismissDisabled()
        .accessibilityIdentifier("p058-drift-review-sheet")
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
            EscalationTraceTimeline(traceJSONRedacted: traceJSONRedacted)
        }
        .padding(16)
        .accessibilityIdentifier("p058-escalation-inspector")
    }
}

