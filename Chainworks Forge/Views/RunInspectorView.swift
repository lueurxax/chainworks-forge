import AppKit
import SwiftUI

/// Full-chrome redesign (macOS 27) — the trailing **Inspector** pane for the Runs surface.
///
/// Mirrors the brand UI kit's `Inspector.jsx`: a contextual panel about the *selected* run
/// with three sections — Run snapshot, Approval gate, Recovery. The view is strictly
/// read-only over existing presentation models; its single mutation routes through the
/// blessed `model.settleApproval(_:action:)` path, preserving the thin-read boundary.
struct RunInspectorView: View {
    @ObservedObject var model: P031ThinReadDashboardModel
    @ObservedObject var workbench: RunsWorkbenchPresentationModel
    /// Opens the existing recovery/closeout surface in the detail pane.
    var onOpenRecovery: () -> Void
    /// Preview-only seam: inject a pending approval so the gate renders in `#Preview`
    /// without standing up the full GraphQL approval-affordance pipeline. Always nil
    /// in production (the gate reads `workbench.inlineApprovals`).
    var previewApprovalOverride: RunsWorkbenchPresentationModel.ApprovalRow? = nil

    /// Approvals the operator chose to "Hold for later" — a local UI affordance only.
    /// Holding leaves the approval pending (no settle), matching the brand's third action.
    @State private var heldApprovalIDs: Set<String> = []

    private var placeholderValue: String { "—" }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: ForgeSpacing.section) {
                if model.selectedRunID == nil && workbench.summaryHeader == nil {
                    emptyState
                } else {
                    runSnapshotSection
                    approvalGateSection
                    outputContractRepairSection
                    recoverySection
                }
            }
            .padding(ForgeSpacing.large)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .accessibilityIdentifier("run-inspector-panel")
    }

    // MARK: - Empty state

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.small) {
            Text("No run selected")
                .font(.headline)
            Text("Select a run to inspect its frozen snapshot, approval gate, and recovery state.")
                .font(.callout)
                .foregroundStyle(ForgeColor.Text.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(ForgeSpacing.large)
        .forgeGlassSurface(.panel)
        .accessibilityIdentifier("run-inspector-empty-state")
    }

    // MARK: - Run snapshot

    private var runSnapshotSection: some View {
        let catalog = workbench.catalogContext
        let agent = workbench.activeTimelineAgents.first
        let rows: [(label: String, value: String, id: String)] = [
            ("Run ID", model.selectedRunID ?? workbench.summaryHeader?.runID ?? placeholderValue, "run"),
            ("Workflow", catalog?.workflowTitle ?? workbench.summaryHeader?.workflowLabel ?? placeholderValue, "workflow"),
            ("Catalog", catalog?.catalogSnapshotHash ?? placeholderValue, "catalog"),
            ("Provider", agent?.providerID ?? placeholderValue, "provider"),
            ("Model", agent?.modelID ?? placeholderValue, "model"),
            // No frozen-snapshot backing field yet — surfaced honestly as "—" until the
            // read boundary plumbs run-plan runtime/start-time (see plan, out of scope here).
            ("Runtime", placeholderValue, "runtime"),
            ("Started", placeholderValue, "started")
        ]

        return InspectorSection(icon: "shield", title: "Run snapshot", subtitle: "Frozen at run start") {
            VStack(spacing: ForgeSpacing.small) {
                ForEach(rows, id: \.id) { row in
                    HStack(alignment: .firstTextBaseline, spacing: ForgeSpacing.small) {
                        Text(row.label.uppercased())
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(ForgeColor.Text.tertiary)
                        Spacer(minLength: ForgeSpacing.small)
                        Text(row.value)
                            .font(.caption.monospaced())
                            .foregroundStyle(ForgeColor.Text.primary)
                            .multilineTextAlignment(.trailing)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    .accessibilityIdentifier("run-snapshot-field-\(row.id)")
                }
            }
        }
        .accessibilityIdentifier("run-snapshot-section")
    }

    // MARK: - Approval gate

    @ViewBuilder
    private var approvalGateSection: some View {
        if let approval = previewApprovalOverride ?? workbench.inlineApprovals.first {
            InspectorSection(icon: "checkmark.seal", title: "Approval gate", subtitle: approval.title) {
                VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                    if let body = approval.body {
                        Text(body)
                            .font(.footnote)
                            .foregroundStyle(ForgeColor.Text.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }

                    if heldApprovalIDs.contains(approval.id) {
                        HStack(spacing: ForgeSpacing.small) {
                            Image(systemName: "pause.circle")
                                .foregroundStyle(ForgeColor.Status.warning)
                            Text("Held for later — the run stays paused.")
                                .font(.footnote)
                                .foregroundStyle(ForgeColor.Text.secondary)
                            Spacer()
                            Button("Resume") { heldApprovalIDs.remove(approval.id) }
                                .buttonStyle(.link)
                        }
                    } else {
                        approvalActions(for: approval)
                    }
                }
            }
            .accessibilityIdentifier("inspector-approval-section")
        }
    }

    @ViewBuilder
    private func approvalActions(for approval: RunsWorkbenchPresentationModel.ApprovalRow) -> some View {
        let isResolving = model.resolvingApprovalIDs.contains(approval.id)

        VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
            HStack(spacing: ForgeSpacing.small) {
                Button {
                    Task { await model.settleApproval(approval.id, action: .approve) }
                } label: {
                    Label("Approve", systemImage: "checkmark")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(ForgeColor.Status.success)
                .disabled(!approval.canApprove || isResolving)
                .help(approval.approveDisabledReason ?? "Approve and continue the run")
                .accessibilityIdentifier("inspector-approval-approve-button")

                Button {
                    Task { await model.settleApproval(approval.id, action: .reject(reason: "inspector_ui_reject")) }
                } label: {
                    Text("Reject")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .disabled(!approval.canReject || isResolving)
                .help(approval.rejectDisabledReason ?? "Reject and stop the run")
                .accessibilityIdentifier("inspector-approval-reject-button")
            }

            Button("Hold for later") { heldApprovalIDs.insert(approval.id) }
                .buttonStyle(.link)
                .disabled(isResolving)
                .accessibilityIdentifier("inspector-approval-hold-button")

            if isResolving {
                ProgressView()
                    .controlSize(.small)
            }
        }
    }

    // MARK: - Output repair

    @ViewBuilder
    private var outputContractRepairSection: some View {
        if let presentation = workbench.outputContractRepair {
            let color = outputContractRepairColor(for: presentation.category)
            InspectorSection(
                icon: presentation.category.sfSymbolName,
                iconColor: color,
                title: "Output repair",
                subtitle: presentation.statusLabel
            ) {
                VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                    GroupBox("Status") {
                        VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                            HStack(spacing: ForgeSpacing.small) {
                                Image(systemName: presentation.category.sfSymbolName)
                                    .foregroundStyle(color)
                                Text(presentation.compactSignalLabel)
                                    .font(.callout.weight(.semibold))
                                    .foregroundStyle(ForgeColor.Text.primary)
                                if presentation.showProgressChip {
                                    ProgressView()
                                        .controlSize(.small)
                                        .accessibilityIdentifier("p079-output-repair-progress")
                                }
                            }
                            .accessibilityLabel(presentation.category.accessibilityLabel(status: outputContractRepairStatus(for: presentation)))
                            .accessibilityIdentifier(presentation.category.accessibilityIdentifier)

                            if presentation.isStale {
                                Text(["Stale", presentation.staleSinceLabel].compactMap { $0 }.joined(separator: " · "))
                                    .font(.caption)
                                    .foregroundStyle(ForgeColor.Status.warning)
                                    .accessibilityIdentifier("p079-output-repair-stale-chip")
                            }

                            if !presentation.diagnosticRows.isEmpty {
                                VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                                    ForEach(presentation.diagnosticRows, id: \.self) { row in
                                        Text(row)
                                            .font(.caption)
                                            .foregroundStyle(ForgeColor.Text.secondary)
                                            .fixedSize(horizontal: false, vertical: true)
                                    }
                                }
                                .accessibilityIdentifier("p079-output-repair-diagnostics")
                            }
                        }
                    }

                    GroupBox("Evidence") {
                        VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                            outputRepairPathRows(
                                title: "Plan evidence",
                                paths: presentation.planEvidencePaths
                            )

                            if let evidencePath = presentation.evidenceArtifactPath {
                                outputRepairPathRow(label: "Evidence", value: evidencePath)
                            }
                        }
                    }

                    GroupBox("Authority") {
                        VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                            if !presentation.permissionDecisions.isEmpty {
                                ForEach(Array(presentation.permissionDecisions.enumerated()), id: \.offset) { _, decision in
                                    Text("\(decision.method) · \(decision.decision.rawValue) · \(decision.reason)")
                                        .font(.caption)
                                        .foregroundStyle(ForgeColor.Text.secondary)
                                        .lineLimit(2)
                                        .textSelection(.enabled)
                                        .accessibilityLabel("\(decision.method), \(decision.decision.rawValue), \(decision.reason)")
                                }
                            }

                            outputRepairCopyRow(label: "Repair attempt", value: presentation.repairAttemptId)
                        }
                    }
                    .accessibilityIdentifier("p079-output-repair-permission-decisions")
                }
            }
            .accessibilityIdentifier("p079-output-contract-repair-section")
        }
    }

    @ViewBuilder
    private func outputRepairPathRows(title: String, paths: [String]) -> some View {
        if !paths.isEmpty {
            VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
                Text(title.uppercased())
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(ForgeColor.Text.tertiary)
                ForEach(paths, id: \.self) { path in
                    outputRepairPathRow(label: "Path", value: path)
                }
            }
            .accessibilityIdentifier("p079-output-repair-plan-evidence")
        }
    }

    private func outputRepairPathRow(label: String, value: String) -> some View {
        outputRepairCopyRow(label: label, value: value)
            .contextMenu {
                Button("Copy Path") { copyToPasteboard(value) }
                Button("Reveal in Finder") { revealOutputRepairPath(value) }
                    .disabled(outputRepairRevealURL(for: value) == nil)
            }
    }

    private func outputRepairCopyRow(label: String, value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: ForgeSpacing.small) {
            Text(label.uppercased())
                .font(.caption2.weight(.semibold))
                .foregroundStyle(ForgeColor.Text.tertiary)
            Spacer(minLength: ForgeSpacing.small)
            Text(value)
                .font(.caption.monospaced())
                .foregroundStyle(ForgeColor.Text.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
                .contextMenu {
                    Button("Copy Path") { copyToPasteboard(value) }
                }
        }
    }

    private func outputContractRepairColor(for category: PresentationCategory) -> Color {
        switch category {
        case .informational: return ForgeColor.Brand.accent
        case .recovered: return ForgeColor.Status.success
        case .blocked, .failed: return ForgeColor.Status.error
        case .skipped, .cancelled, .unknownDiagnostic: return ForgeColor.Text.tertiary
        }
    }

    private func outputContractRepairStatus(
        for presentation: OutputContractRepairPresentation
    ) -> OutputContractRepairStatus {
        switch presentation.category {
        case .informational:
            return presentation.showProgressChip ? .inProgress : .notAttempted
        case .recovered: return .recovered
        case .blocked: return .blocked
        case .skipped: return .skipped
        case .failed: return .failed
        case .cancelled: return .cancelled
        case .unknownDiagnostic(let raw): return .unknownDiagnostic(raw)
        }
    }

    private func copyToPasteboard(_ value: String) {
        ArtifactPathClipboard.copy(path: value)
    }

    private func revealOutputRepairPath(_ value: String) {
        guard let url = outputRepairRevealURL(for: value) else { return }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    private func outputRepairRevealURL(for value: String) -> URL? {
        guard OutputContractRepairPresenter.safeRelativePath(value) == value else { return nil }
        let runID = model.selectedRunID ?? workbench.summaryHeader?.runID
        let base = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let candidates = [
            runID.map { base.appendingPathComponent(".chainworks/runs/\($0)", isDirectory: true) },
            Optional(base.appendingPathComponent(".chainworks", isDirectory: true))
        ].compactMap { $0 }

        return candidates
            .map { $0.appendingPathComponent(value) }
            .first { FileManager.default.fileExists(atPath: $0.path) }
    }

    // MARK: - Recovery


    private var recoverySection: some View {
        // Per-row status is not exposed by the read boundary (`diagnosticRows` are flat
        // strings); the closeout readiness carries a single overall `visualState`. Drive the
        // row/section iconography from that real state instead of a hardcoded green check, so
        // a warning/blocking readiness no longer renders misleading success ticks.
        let style = recoveryStateStyle
        return InspectorSection(
            icon: style.icon,
            iconColor: style.color,
            title: "Recovery",
            subtitle: "Resume from last sealed checkpoint"
        ) {
            VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                if workbench.recoveryEvidence.isEmpty {
                    Text("No recovery checkpoints reported yet.")
                        .font(.footnote)
                        .foregroundStyle(ForgeColor.Text.secondary)
                } else {
                    ForEach(workbench.recoveryEvidence) { row in
                        HStack(alignment: .top, spacing: ForgeSpacing.small) {
                            Image(systemName: style.rowIcon)
                                .font(.caption.weight(.bold))
                                .foregroundStyle(style.color)
                            Text(row.title)
                                .font(.callout)
                                .foregroundStyle(ForgeColor.Text.primary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }

                Button("Open recovery sheet", action: onOpenRecovery)
                    .accessibilityIdentifier("inspector-recovery-open-sheet-button")
            }
        }
        .accessibilityIdentifier("inspector-recovery-section")
    }

    /// Maps the overall closeout `visualState` to honest iconography. `rowIcon` is the
    /// per-row leading glyph; `icon` is the section header glyph; `color` applies to both.
    private var recoveryStateStyle: (icon: String, rowIcon: String, color: Color) {
        switch workbench.closeoutReadiness?.visualState {
        case .positive:
            return ("checkmark.circle.fill", "checkmark", ForgeColor.Status.success)
        case .warning:
            return ("exclamationmark.triangle.fill", "exclamationmark", ForgeColor.Status.warning)
        case .blocking:
            return ("xmark.octagon.fill", "xmark", ForgeColor.Status.error)
        case .neutral, .none:
            return ("clock.arrow.circlepath", "circle.fill", ForgeColor.Text.tertiary)
        }
    }
}

/// A titled, glass-backed panel matching the brand `fg-panel` + `SectionHeader` idiom.
private struct InspectorSection<Content: View>: View {
    let icon: String
    var iconColor: Color = ForgeColor.Brand.accent
    let title: String
    var subtitle: String?
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
            HStack(spacing: ForgeSpacing.small) {
                Image(systemName: icon)
                    .foregroundStyle(iconColor)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(ForgeColor.Text.primary)
                    if let subtitle {
                        Text(subtitle)
                            .font(.caption2)
                            .foregroundStyle(ForgeColor.Text.tertiary)
                            .lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
            }

            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(ForgeSpacing.large)
        .forgeGlassSurface(.panel)
    }
}

#if DEBUG
private struct RunInspectorPreviewHost: View {
    @StateObject private var model = P031ThinReadDashboardModel.previewLoadedWithCloseoutReadiness()
    @StateObject private var workbench = RunsWorkbenchPresentationModel()

    private let sampleApproval = RunsWorkbenchPresentationModel.ApprovalRow(
        id: "preview-approval-1",
        title: "Reviewer submitted a proposal",
        body: "The reviewer flagged 3 medium-severity findings on stage settlement. Continue the run, reject, or hold for later.",
        canApprove: true,
        canReject: true,
        approveDisabledReason: nil,
        rejectDisabledReason: nil,
        deferredState: nil,
        accessibilityLabel: "Approval pending review",
        followUpID: nil,
        copyItems: []
    )

    var body: some View {
        RunInspectorView(
            model: model,
            workbench: workbench,
            onOpenRecovery: {},
            previewApprovalOverride: sampleApproval
        )
        .frame(width: 320, height: 880)
        .onAppear {
            if let runsHome = model.runsHome { workbench.populate(from: runsHome) }
            if let runDetail = model.runDetail { workbench.populate(from: runDetail) }
            workbench.populate(daemon: model.daemonLifecycle, scheduler: model.schedulerHealth)
        }
    }
}

#Preview("Run Inspector") {
    RunInspectorPreviewHost()
}
#endif
