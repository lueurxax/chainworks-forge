import SwiftUI
import SwiftData
import AppKit

// MARK: - ReleaseGateView (Proposal 007 — §10.3)

/// Dedicated release gate view for repo-backed manual release approval.
/// Shows enough context for an informed approval decision:
/// proposal summary, review summary status, changed files/diff stat, tests result,
/// security/audit/docs summary, target branch, release destination.
/// Quick actions: open proposal, open diff, approve, reject.
struct ReleaseGateView: View {
    fileprivate enum FocusTarget: String {
        case openProposal
        case rejectRelease
        case approveRelease

        var label: String {
            switch self {
            case .openProposal: return "Open Proposal"
            case .rejectRelease: return "Reject Release"
            case .approveRelease: return "Approve Release"
            }
        }
    }

    let run: Run
    let onApprove: () -> Void
    let onReject: () -> Void

    @FocusState private var focusedTarget: FocusTarget?

    private var deliveryConfig: DeliveryConfiguration? {
        guard let data = run.deliveryConfigurationJSON else { return nil }
        return try? JSONDecoder().decode(DeliveryConfiguration.self, from: data)
    }

    private var allArtifacts: [Artifact] {
        run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)
    }

    private var focusProofEnabled: Bool {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_FOCUS_PROOF"] == "1"
    }

    private var initialFocusTarget: FocusTarget {
        artifact(named: "approved_proposal") != nil ? .openProposal : .rejectRelease
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            HStack {
                Image(systemName: "shippingbox.fill")
                    .font(.title2)
                    .foregroundStyle(DesignTokens.Action.caution)
                ForgeSectionHeader(
                    title: "Manual Release Gate",
                    subtitle: "Review context, receipts, and release destination before approving a repo-backed delivery handoff.",
                    tint: DesignTokens.Action.caution
                )
                Spacer()
                statusBadge
            }

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Above-the-fold: diff stat, spend, and change summary
                    diffStatAndSpendSection

                    // Proposal summary
                    releaseContextSection

                    // Review summary
                    reviewSummarySection

                    // Repository context
                    repoContextSection

                    // Release target
                    releaseTargetSection

                    // Quick actions for decision context
                    quickActionsSection
                }
                .padding(.bottom, 20)
            }

            Divider()

            // Actions
            HStack(spacing: 12) {
                Button(role: .destructive) {
                    onReject()
                } label: {
                    Label("Reject Release", systemImage: "xmark.circle")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                // Proposal 012 (L-09): Keyboard shortcut for reject
                .keyboardShortcut(.delete, modifiers: [.command])
                .accessibilityIdentifier("release-gate-reject-button")
                .focusable(focusProofEnabled)
                .focused($focusedTarget, equals: .rejectRelease)

                Button {
                    onApprove()
                } label: {
                    Label("Approve Release", systemImage: "checkmark.circle.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(DesignTokens.Action.caution)
                // Proposal 012 (L-09): Keyboard shortcut for approve
                .keyboardShortcut(.return, modifiers: [.command])
                .accessibilityIdentifier("release-gate-approve-button")
                .focusable(focusProofEnabled)
                .focused($focusedTarget, equals: .approveRelease)
            }
            .controlSize(.large)
        }
        .padding()
        .frame(minWidth: 500, minHeight: 400)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("release-gate-view")
        .overlay(alignment: .topLeading) {
            if focusProofEnabled {
                Text("Focused: \(focusedTarget?.label ?? "None")")
                    .font(.caption2)
                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    .padding(.top, 2)
                    .padding(.leading, 2)
                    .accessibilityIdentifier("release-gate-focus-order")
            }
        }
        .task(id: focusProofEnabled) {
            guard focusProofEnabled else { return }
            await MainActor.run {
                focusedTarget = initialFocusTarget
            }
        }
    }

// MARK: - Sections

    @ViewBuilder
    private var statusBadge: some View {
        StatusCapsule(
            text: "Awaiting Approval",
            color: DesignTokens.Status.warning,
            icon: "checkmark.seal",
            accessibilityIdentifier: "release-gate-status-badge"
        )
    }

    @ViewBuilder
    private var releaseContextSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                ForgeSectionHeader(title: "Release Context", systemImage: "doc.text")

                LabeledContent("Workflow") {
                    Text(run.workflowTitle)
                }
                LabeledContent("Idea") {
                    Text(run.idea?.title ?? "—")
                }
                if let config = deliveryConfig {
                    LabeledContent("Profile") {
                        Text(config.profileLabel ?? "Direct")
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var reviewSummarySection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                ForgeSectionHeader(title: "Review Summary", systemImage: "checklist")

                reviewRow("Audit Report", artifactName: "audit_report", in: allArtifacts)
                reviewRow("Security Report", artifactName: "security_report", in: allArtifacts)
                reviewRow("Pre-Push Review", artifactName: "prepush_review_report", in: allArtifacts)
                reviewRow("Docs Report", artifactName: "docs_report", in: allArtifacts)
                reviewRow("Tests Result", artifactName: "tests_result", in: allArtifacts)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    // Proposal 012 (L-06): Three-state review row semantics.
    // - "Available" (green) — artifact exists
    // - "Not yet produced" (neutral/secondary) — expected during in-progress runs
    // - "Missing" (warning/orange) — expected artifact was not generated in a terminal run
    @ViewBuilder
    private func reviewRow(_ label: String, artifactName: String, in artifacts: [Artifact]) -> some View {
        let exists = artifacts.contains { $0.name == artifactName }
        let isTerminal = run.status == .completed || run.status == .failed || run.status == .cancelled
        HStack {
            if exists {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(DesignTokens.Status.success)
            } else if isTerminal {
                Image(systemName: "exclamationmark.circle.fill")
                    .foregroundStyle(DesignTokens.Status.warning)
            } else {
                Image(systemName: "circle.dashed")
                    .foregroundStyle(DesignTokens.Status.neutral)
            }
            Text(label)
            Spacer()
            if exists {
                StatusCapsule(text: "Available", color: DesignTokens.Status.success, size: .small)
            } else if isTerminal {
                StatusCapsule(text: "Missing", color: DesignTokens.Status.warning, size: .small)
            } else {
                Text("Not yet produced")
                    .font(DesignTokens.Typography.supporting)
                    .foregroundStyle(DesignTokens.Status.neutral)
            }
        }
    }

    @ViewBuilder
    private var repoContextSection: some View {
        if let config = deliveryConfig {
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    ForgeSectionHeader(title: "Repository", systemImage: "arrow.triangle.branch")

                    LabeledContent("Repo") {
                        Text(config.repoIdentifier)
                            .font(.system(.body, design: .monospaced))
                    }
                    LabeledContent("Base Branch") {
                        Text(config.baseBranch)
                            .font(.system(.body, design: .monospaced))
                    }
                    LabeledContent("Target Branch") {
                        Text(config.targetBranch)
                            .font(.system(.body, design: .monospaced))
                    }
                    if let baseRev = run.baseRevision {
                        LabeledContent("Base Revision") {
                            Text(String(baseRev.prefix(8)))
                                .font(.system(.body, design: .monospaced))
                        }
                    }
                    if let worktree = run.worktreeRoot {
                        VStack(alignment: .leading, spacing: 6) {
                            LabeledContent("Worktree") {
                                Text(worktree)
                                    .font(.caption)
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                            }
                            Button("Open Worktree in Finder") {
                                NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: worktree)
                            }
                            .buttonStyle(.link)
                            .accessibilityIdentifier("release-gate-open-worktree")
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    @ViewBuilder
    private var diffStatAndSpendSection: some View {
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    ForgeSectionHeader(title: "Change Summary", systemImage: "chart.bar.doc.horizontal")

                let allArtifacts = run.stageExecutions
                    .flatMap(\.agentExecutions)
                    .flatMap(\.artifacts)

                // Diff stat
                if allArtifacts.contains(where: { $0.name == "changed_files_manifest" }) {
                    LabeledContent("Changed Files") {
                        Text("Available")
                            .font(.caption)
                            .foregroundStyle(DesignTokens.Status.success)
                    }
                } else {
                    LabeledContent("Changed Files") {
                        Text("Not yet produced")
                            .font(.caption)
                            .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    }
                }

                // Implementation loop iterations
                if let implCount = run.loopCounters["implementation_progress_count"] {
                    LabeledContent("Implementation Iterations") {
                        Text("\(implCount)")
                    }
                }
                if let revisionCount = run.loopCounters["implementation_revision_count"] {
                    LabeledContent("Refinement Cycles") {
                        Text("\(revisionCount)")
                    }
                }

                // Spend
                LabeledContent("Spend to Date") {
                    if let cost = run.totalCostCents {
                        Text("\(cost) cents")
                            .font(.system(.body, design: .monospaced))
                    } else {
                        Text("Estimated / unavailable")
                            .font(.caption)
                            .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    }
                }

                // Elapsed time
                let elapsed = run.completedAt ?? Date()
                let duration = elapsed.timeIntervalSince(run.startedAt)
                let minutes = Int(duration) / 60
                let seconds = Int(duration) % 60
                LabeledContent("Elapsed") {
                    Text("\(minutes)m \(seconds)s")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var quickActionsSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Label("Decision Context", systemImage: "doc.text.magnifyingglass")
                    .font(DesignTokens.Typography.sectionHeader)

                let contextArtifacts: [(name: String, label: String, icon: String)] = [
                    ("approved_proposal", "Open Proposal", "doc.text"),
                    ("changed_files_manifest", "Open Diff Summary", "doc.text.magnifyingglass"),
                    ("docs_delta", "Open Docs Delta", "doc.richtext"),
                    ("implementation_review_summary", "Open Review Summary", "checklist"),
                    ("security_report", "Open Security Report", "lock.shield"),
                    ("audit_report", "Open Audit Report", "checkmark.rectangle.stack"),
                    ("prepush_review_report", "Open Pre-Push Review", "arrow.up.doc"),
                    ("delivery_receipt", "Open Receipts/Report", "doc.badge.checkmark")
                ]

                ForEach(contextArtifacts, id: \.name) { item in
                    if let artifact = artifact(named: item.name) {
                        if focusProofEnabled && item.name == "approved_proposal" {
                            Button {
                                openArtifact(artifact)
                            } label: {
                                HStack {
                                    Label(item.label, systemImage: item.icon)
                                    Spacer()
                                    Image(systemName: "chevron.right")
                                        .font(.caption2)
                                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                }
                                .font(.callout)
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("release-gate-open-\(item.name)")
                            .focusable()
                            .focused($focusedTarget, equals: .openProposal)
                        } else {
                            Button {
                                openArtifact(artifact)
                            } label: {
                                HStack {
                                    Label(item.label, systemImage: item.icon)
                                    Spacer()
                                    Image(systemName: "chevron.right")
                                        .font(.caption2)
                                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                }
                                .font(.callout)
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("release-gate-open-\(item.name)")
                        }
                    } else {
                        HStack {
                            Label(item.label, systemImage: item.icon)
                                .foregroundStyle(DesignTokens.Neutral.textSecondary)
                            Spacer()
                            Text("—")
                                .font(.caption)
                                .foregroundStyle(DesignTokens.Neutral.textTertiary)
                        }
                        .font(.callout)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("release-gate-decision-context")
    }

    @ViewBuilder
    private var releaseTargetSection: some View {
        if let config = deliveryConfig {
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    Label("Release Target", systemImage: "shippingbox")
                        .font(DesignTokens.Typography.sectionHeader)

                    LabeledContent("Target") {
                        Text(config.releaseTargetLabel)
                    }
                    LabeledContent("Mode") {
                        StatusCapsule(
                            text: config.releaseMode.rawValue.capitalized,
                            color: config.releaseMode == .sandbox ? DesignTokens.Action.primary : DesignTokens.Action.caution,
                            size: .small,
                            accessibilityIdentifier: "release-gate-release-mode"
                        )
                    }
                    LabeledContent("Safety") {
                        Text("Dedicated worktree \u{00B7} Manual gate \u{00B7} Deterministic services")
                            .font(.caption)
                            .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func artifact(named name: String) -> Artifact? {
        allArtifacts.last(where: { $0.name == name })
    }

    private func openArtifact(_ artifact: Artifact) {
        let url = URL(fileURLWithPath: artifact.filePath)
        guard FileManager.default.fileExists(atPath: artifact.filePath) else { return }
        NSWorkspace.shared.open(url)
    }
}

// MARK: - Preview

private func makePreviewReleaseGateRun(in container: ModelContainer) -> Run {
    let context = container.mainContext
    let descriptor = FetchDescriptor<Run>()
    let runs = (try? context.fetch(descriptor)) ?? []
    let run = runs.first(where: { $0.status == .waitingApproval }) ?? runs.first!

    let config = DeliveryConfiguration(
        profileID: "dogfood_self",
        profileLabel: "Self (Dogfood)",
        sampleProfileID: nil,
        repoIdentifier: "user/chainworks-forge",
        repoRoot: "/Users/user/Documents/Chainworks Forge",
        baseBranch: "main",
        worktreeBasePath: "/Users/user/Library/Application Support/Chainworks Forge/worktrees",
        targetBranch: "release/proposal-007",
        releaseTargetID: "sandbox_local",
        releaseTargetLabel: "Local Sandbox",
        releaseMode: .sandbox
    )
    run.deliveryConfigurationJSON = try? JSONEncoder().encode(config)
    run.baseRevision = "e1655a6b"
    run.worktreeRoot = "/Users/user/Library/Application Support/Chainworks Forge/worktrees/run-abc123"
    return run
}

#Preview("Release Gate — Sandbox") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let run = makePreviewReleaseGateRun(in: container)

    ReleaseGateView(
        run: run,
        onApprove: {},
        onReject: {}
    )
    .modelContainer(container)
    .frame(width: 600, height: 700)
}
