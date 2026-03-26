import SwiftUI
import SwiftData

// MARK: - ReleaseGateView (Proposal 007 — §10.3)

/// Dedicated release gate view for repo-backed manual release approval.
/// Shows enough context for an informed approval decision:
/// proposal summary, review summary status, changed files/diff stat, tests result,
/// security/audit/docs summary, target branch, release destination.
/// Quick actions: open proposal, open diff, approve, reject.
struct ReleaseGateView: View {
    let run: Run
    let onApprove: () -> Void
    let onReject: () -> Void

    private var deliveryConfig: DeliveryConfiguration? {
        guard let data = run.deliveryConfigurationJSON else { return nil }
        return try? JSONDecoder().decode(DeliveryConfiguration.self, from: data)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            HStack {
                Image(systemName: "shippingbox.fill")
                    .font(.title2)
                    .foregroundStyle(.orange)
                Text("Manual Release Gate")
                    .font(.title2.bold())
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
                .accessibilityIdentifier("release-gate-reject-button")

                Button {
                    onApprove()
                } label: {
                    Label("Approve Release", systemImage: "checkmark.circle.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(.orange)
                .accessibilityIdentifier("release-gate-approve-button")
            }
            .controlSize(.large)
        }
        .padding()
        .frame(minWidth: 500, minHeight: 400)
        .accessibilityIdentifier("release-gate-view")
    }

    // MARK: - Sections

    @ViewBuilder
    private var statusBadge: some View {
        Text("Awaiting Approval")
            .font(.caption.bold())
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(.orange.opacity(0.15))
            .foregroundStyle(.orange)
            .clipShape(Capsule())
    }

    @ViewBuilder
    private var releaseContextSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Label("Release Context", systemImage: "doc.text")
                    .font(.headline)

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
                Label("Review Summary", systemImage: "checklist")
                    .font(.headline)

                let allArtifacts = run.stageExecutions
                    .flatMap(\.agentExecutions)
                    .flatMap(\.artifacts)

                reviewRow("Audit Report", artifactName: "audit_report", in: allArtifacts)
                reviewRow("Security Report", artifactName: "security_report", in: allArtifacts)
                reviewRow("Pre-Push Review", artifactName: "prepush_review_report", in: allArtifacts)
                reviewRow("Docs Report", artifactName: "docs_report", in: allArtifacts)
                reviewRow("Tests Result", artifactName: "tests_result", in: allArtifacts)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private func reviewRow(_ label: String, artifactName: String, in artifacts: [Artifact]) -> some View {
        let exists = artifacts.contains { $0.name == artifactName }
        HStack {
            Image(systemName: exists ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(exists ? .green : .secondary)
            Text(label)
            Spacer()
            Text(exists ? "Available" : "Missing")
                .font(.caption)
                .foregroundStyle(exists ? .green : .orange)
        }
    }

    @ViewBuilder
    private var repoContextSection: some View {
        if let config = deliveryConfig {
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    Label("Repository", systemImage: "arrow.triangle.branch")
                        .font(.headline)

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
                        LabeledContent("Worktree") {
                            Text(worktree)
                                .font(.caption)
                                .lineLimit(1)
                                .truncationMode(.middle)
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
                Label("Change Summary", systemImage: "chart.bar.doc.horizontal")
                    .font(.headline)

                let allArtifacts = run.stageExecutions
                    .flatMap(\.agentExecutions)
                    .flatMap(\.artifacts)

                // Diff stat
                if let changedFiles = allArtifacts.first(where: { $0.name == "changed_files_manifest" }) {
                    LabeledContent("Changed Files") {
                        Text("Available")
                            .font(.caption)
                            .foregroundStyle(.green)
                    }
                } else {
                    LabeledContent("Changed Files") {
                        Text("Not yet produced")
                            .font(.caption)
                            .foregroundStyle(.secondary)
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
                            .foregroundStyle(.secondary)
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
                    .font(.headline)

                let allArtifacts = run.stageExecutions
                    .flatMap(\.agentExecutions)
                    .flatMap(\.artifacts)

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
                    let exists = allArtifacts.contains { $0.name == item.name }
                    HStack {
                        Label(item.label, systemImage: item.icon)
                            .foregroundStyle(exists ? .primary : .secondary)
                        Spacer()
                        if exists {
                            Image(systemName: "chevron.right")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        } else {
                            Text("—")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }
                    .font(.callout)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var releaseTargetSection: some View {
        if let config = deliveryConfig {
            GroupBox {
                VStack(alignment: .leading, spacing: 8) {
                    Label("Release Target", systemImage: "shippingbox")
                        .font(.headline)

                    LabeledContent("Target") {
                        Text(config.releaseTargetLabel)
                    }
                    LabeledContent("Mode") {
                        Text(config.releaseMode.rawValue.capitalized)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(config.releaseMode == .sandbox ? Color.blue.opacity(0.15) : Color.orange.opacity(0.15))
                            .clipShape(Capsule())
                    }
                    LabeledContent("Safety") {
                        Text("Dedicated worktree \u{00B7} Manual gate \u{00B7} Deterministic services")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
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
