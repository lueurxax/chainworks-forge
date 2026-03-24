import SwiftUI

// MARK: - P005-OPS §8: Run Comparison View

/// Deterministic structural comparison UI for compatible runs.
/// Shows snapshot, timing, cost, approval, and trust deltas.
/// Does NOT claim repo-backed or release-specific diff support (§8.3).
struct RunComparisonView: View {
    let runA: Run
    let runB: Run
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss
    @State private var comparison: RunComparison?

    var body: some View {
        NavigationStack {
            ScrollView {
                if let comparison {
                    VStack(alignment: .leading, spacing: 16) {
                        comparisonHeader(comparison)
                        snapshotSection(comparison)
                        trustSection(comparison)
                        timingCostSection(comparison)
                        stageSection(comparison)
                        approvalSection(comparison)
                        artifactSection(comparison)
                    }
                    .padding()
                } else {
                    ContentUnavailableView(
                        "Incompatible Runs",
                        systemImage: "xmark.circle",
                        description: Text("These runs cannot be compared.")
                    )
                }
            }
            .navigationTitle("Run Comparison")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .frame(minWidth: 600, minHeight: 500)
        .task {
            let service = RunComparisonService(modelContext: modelContext)
            comparison = service.compare(runA, runB)
        }
    }

    // MARK: - Sections

    @ViewBuilder
    private func comparisonHeader(_ c: RunComparison) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(c.ideaTitle)
                .font(.title2)
            HStack {
                Label("Run A", systemImage: "a.circle.fill")
                    .font(.headline)
                    .foregroundStyle(.blue)
                Text(c.runA_ID.uuidString.prefix(8))
                    .font(.caption.monospaced())
                Spacer()
                Label("Run B", systemImage: "b.circle.fill")
                    .font(.headline)
                    .foregroundStyle(.purple)
                Text(c.runB_ID.uuidString.prefix(8))
                    .font(.caption.monospaced())
            }
        }
    }

    @ViewBuilder
    private func snapshotSection(_ c: RunComparison) -> some View {
        GroupBox("Snapshot") {
            VStack(alignment: .leading, spacing: 4) {
                deltaRow("Workflow Hash", match: c.workflowHashMatch)
                deltaRow("Catalog Hash", match: c.catalogHashMatch)
                if let dA = c.driftA {
                    LabeledContent("Drift A", value: dA)
                }
                if let dB = c.driftB {
                    LabeledContent("Drift B", value: dB)
                }
            }
        }
    }

    @ViewBuilder
    private func trustSection(_ c: RunComparison) -> some View {
        GroupBox("Runtime Trust") {
            HStack {
                VStack(alignment: .leading) {
                    Text("Run A")
                        .font(.caption.bold())
                    RuntimeProvenanceBadge(trustLevel: c.trustLevelA)
                }
                Spacer()
                VStack(alignment: .leading) {
                    Text("Run B")
                        .font(.caption.bold())
                    RuntimeProvenanceBadge(trustLevel: c.trustLevelB)
                }
            }
        }
    }

    @ViewBuilder
    private func timingCostSection(_ c: RunComparison) -> some View {
        GroupBox("Timing & Cost") {
            VStack(alignment: .leading, spacing: 4) {
                comparisonRow("Duration", valueA: formatDuration(c.durationA), valueB: formatDuration(c.durationB), delta: formatDelta(c.durationDelta, suffix: "s"))
                comparisonRow("Cost", valueA: "\(c.costA)c", valueB: "\(c.costB)c", delta: "\(c.costDelta > 0 ? "+" : "")\(c.costDelta)c")
                comparisonRow("Loops", valueA: "\(c.loopsA)", valueB: "\(c.loopsB)", delta: "\(c.loopDelta > 0 ? "+" : "")\(c.loopDelta)")
            }
        }
    }

    @ViewBuilder
    private func stageSection(_ c: RunComparison) -> some View {
        GroupBox("Stage Delta") {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(c.stageDelta) { delta in
                    HStack {
                        Image(systemName: delta.changed ? "arrow.triangle.2.circlepath" : "equal.circle")
                            .foregroundStyle(delta.changed ? .orange : .green)
                        Text(delta.stageID)
                            .font(.caption.monospaced())
                        Spacer()
                        Text(delta.statusA ?? "—")
                            .font(.caption)
                            .foregroundStyle(.blue)
                        Text("→")
                            .font(.caption)
                        Text(delta.statusB ?? "—")
                            .font(.caption)
                            .foregroundStyle(.purple)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func approvalSection(_ c: RunComparison) -> some View {
        GroupBox("Approvals") {
            VStack(alignment: .leading, spacing: 4) {
                comparisonRow("Requested", valueA: "\(c.approvalDelta.requestedA)", valueB: "\(c.approvalDelta.requestedB)", delta: nil)
                comparisonRow("Granted", valueA: "\(c.approvalDelta.grantedA)", valueB: "\(c.approvalDelta.grantedB)", delta: nil)
                comparisonRow("Rejected", valueA: "\(c.approvalDelta.rejectedA)", valueB: "\(c.approvalDelta.rejectedB)", delta: nil)
            }
        }
    }

    @ViewBuilder
    private func artifactSection(_ c: RunComparison) -> some View {
        if !c.pinnedArtifactDiff.isEmpty {
            GroupBox("Pinned Artifacts") {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(c.pinnedArtifactDiff) { delta in
                        HStack {
                            Image(systemName: artifactDeltaIcon(delta))
                                .foregroundStyle(artifactDeltaColor(delta))
                            Text(delta.name)
                                .font(.caption.monospaced())
                            Spacer()
                            if let match = delta.contentMatch {
                                Text(match ? "identical" : "different")
                                    .font(.caption)
                                    .foregroundStyle(match ? .green : .orange)
                            } else {
                                if !delta.presentInA { Text("only in B").font(.caption).foregroundStyle(.purple) }
                                if !delta.presentInB { Text("only in A").font(.caption).foregroundStyle(.blue) }
                            }
                        }
                    }
                }
            }
        }
    }

    // MARK: - Helpers

    private func deltaRow(_ label: String, match: Bool) -> some View {
        HStack {
            Text(label)
            Spacer()
            Image(systemName: match ? "checkmark.circle.fill" : "xmark.circle.fill")
                .foregroundStyle(match ? .green : .orange)
            Text(match ? "Match" : "Different")
                .font(.caption)
                .foregroundStyle(match ? .green : .orange)
        }
    }

    private func comparisonRow(_ label: String, valueA: String, valueB: String, delta: String?) -> some View {
        HStack {
            Text(label)
                .frame(width: 80, alignment: .leading)
            Text(valueA)
                .foregroundStyle(.blue)
                .frame(width: 80, alignment: .trailing)
            Text("vs")
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(valueB)
                .foregroundStyle(.purple)
                .frame(width: 80, alignment: .trailing)
            if let delta {
                Text(delta)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .font(.caption.monospaced())
    }

    private func formatDuration(_ seconds: Double) -> String {
        let mins = Int(seconds) / 60
        let secs = Int(seconds) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }

    private func formatDelta(_ value: Double, suffix: String) -> String {
        let sign = value > 0 ? "+" : ""
        return "\(sign)\(Int(value))\(suffix)"
    }

    private func artifactDeltaIcon(_ delta: RunComparison.PinnedArtifactDelta) -> String {
        if let match = delta.contentMatch {
            return match ? "equal.circle.fill" : "arrow.triangle.2.circlepath"
        }
        return "plus.circle"
    }

    private func artifactDeltaColor(_ delta: RunComparison.PinnedArtifactDelta) -> Color {
        if let match = delta.contentMatch {
            return match ? .green : .orange
        }
        return .blue
    }
}
