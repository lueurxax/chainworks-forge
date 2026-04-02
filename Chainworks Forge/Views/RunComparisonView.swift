import SwiftUI
import SwiftData

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
                        strategySection(comparison)
                        snapshotSection(comparison)
                        trustSection(comparison)
                        bindingsSection(comparison)
                        proposalFeedbackSection(comparison)
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
            HStack(spacing: 8) {
                ParentIdeaArchiveBadge(title: "Run A parent", idea: runA.idea)
                ParentIdeaArchiveBadge(title: "Run B parent", idea: runB.idea)
            }
            HStack(spacing: 8) {
                StrategyBadge(
                    profileID: c.strategyComparison.profileA,
                    assignmentMode: c.strategyComparison.assignmentModeA,
                    recommendationState: runA.strategyRecommendationState
                )
                StrategyBadge(
                    profileID: c.strategyComparison.profileB,
                    assignmentMode: c.strategyComparison.assignmentModeB,
                    recommendationState: runB.strategyRecommendationState
                )
            }
        }
    }

    @ViewBuilder
    private func strategySection(_ c: RunComparison) -> some View {
        GroupBox("Strategy (Proposal 019)") {
            VStack(alignment: .leading, spacing: 6) {
                comparisonRow("Profile A", valueA: c.strategyComparison.profileA ?? "—", valueB: c.strategyComparison.profileB ?? "—", delta: nil)
                comparisonRow("Assign Mode", valueA: c.strategyComparison.assignmentModeA ?? "—", valueB: c.strategyComparison.assignmentModeB ?? "—", delta: nil)
                comparisonRow("Evidence", valueA: c.strategyComparison.evidenceComplete ? "complete" : "incomplete", valueB: " ", delta: nil)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Divider()
                HStack(alignment: .top, spacing: 8) {
                    Text("Recommendation")
                        .font(.caption.monospaced())
                    Spacer()
                    Text(c.strategyRecommendation.status.rawValue)
                    .font(.caption.monospaced())
                    .foregroundStyle(strategyColor(c.strategyRecommendation.status))
                }
                Text("Proof owner: \(c.strategyRecommendation.proofOwner)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Text("Evaluation set: \(c.strategyRecommendation.evaluationSetSummary)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                if let recommendedProfile = c.strategyRecommendation.recommendedProfileID {
                    Text("Recommended profile: \(recommendedProfile)")
                        .font(.caption)
                }
                if let quality = c.strategyComparison.qualityDeltaSummary {
                    Text(quality)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                if !c.strategyRecommendation.holdCriteria.isEmpty {
                    Text("Hold criteria: \(c.strategyRecommendation.holdCriteria.joined(separator: ", "))")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
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

    // MARK: - Provider/Model/Effort Bindings (§8.2)

    @ViewBuilder
    private func bindingsSection(_ c: RunComparison) -> some View {
        if !c.bindingsA.isEmpty || !c.bindingsB.isEmpty {
            GroupBox("Provider / Model / Effort Bindings") {
                let allAgentIDs = Set(c.bindingsA.map(\.agentID)).union(c.bindingsB.map(\.agentID)).sorted()
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(allAgentIDs, id: \.self) { agentID in
                        let bindingA = c.bindingsA.first(where: { $0.agentID == agentID })
                        let bindingB = c.bindingsB.first(where: { $0.agentID == agentID })
                        let changed = bindingA?.provider != bindingB?.provider
                            || bindingA?.model != bindingB?.model
                            || bindingA?.effort != bindingB?.effort
                        HStack {
                            Image(systemName: changed ? "arrow.triangle.2.circlepath" : "equal.circle")
                                .foregroundStyle(changed ? .orange : .green)
                            Text(agentID)
                                .font(.caption.monospaced().bold())
                                .frame(width: 100, alignment: .leading)
                            VStack(alignment: .leading, spacing: 1) {
                                HStack(spacing: 4) {
                                    Text("A:")
                                        .font(.caption2)
                                        .foregroundStyle(.blue)
                                    Text(bindingSummary(bindingA))
                                        .font(.caption2.monospaced())
                                        .foregroundStyle(.blue)
                                    // Proposal 011 (REQ-010): Cross-family warning for run A.
                                    if bindingA?.hasCrossFamilyMismatch == true {
                                        Image(systemName: "exclamationmark.triangle.fill")
                                            .foregroundStyle(.yellow)
                                            .help("Cross-family binding mismatch")
                                    }
                                }
                                HStack(spacing: 4) {
                                    Text("B:")
                                        .font(.caption2)
                                        .foregroundStyle(.purple)
                                    Text(bindingSummary(bindingB))
                                        .font(.caption2.monospaced())
                                        .foregroundStyle(.purple)
                                    // Proposal 011 (REQ-010): Cross-family warning for run B.
                                    if bindingB?.hasCrossFamilyMismatch == true {
                                        Image(systemName: "exclamationmark.triangle.fill")
                                            .foregroundStyle(.yellow)
                                            .help("Cross-family binding mismatch")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func proposalFeedbackSection(_ c: RunComparison) -> some View {
        GroupBox("Proposal-loop feedback fidelity (Proposal 022)") {
            VStack(alignment: .leading, spacing: 6) {
                comparisonNumberRow(
                    "Corpus bundle",
                    valueA: c.proposalLoopComparison.reviewCorpusBundlePresentA.map { $0 ? "present" : "missing" } ?? "—",
                    valueB: c.proposalLoopComparison.reviewCorpusBundlePresentB.map { $0 ? "present" : "missing" } ?? "—"
                )
                comparisonNumberRow(
                    "Merge provenance",
                    valueA: c.proposalLoopComparison.mergeProvenanceItemCountA.map { "\($0)" } ?? "—",
                    valueB: c.proposalLoopComparison.mergeProvenanceItemCountB.map { "\($0)" } ?? "—"
                )
                comparisonNumberRow(
                    "Backlog A",
                    valueA: c.proposalLoopComparison.backlogItemCountA.map { "\($0)" } ?? "—",
                    valueB: c.proposalLoopComparison.backlogItemCountB.map { "\($0)" } ?? "—"
                )
                comparisonNumberRow(
                    "Unresolved A",
                    valueA: c.proposalLoopComparison.unresolvedItemCountA.map { "\($0)" } ?? "—",
                    valueB: c.proposalLoopComparison.unresolvedItemCountB.map { "\($0)" } ?? "—"
                )
                if let unresolvedDelta = c.proposalLoopComparison.unresolvedDelta {
                    Text("Unresolved delta: \(unresolvedDelta > 0 ? "+" : "")\(unresolvedDelta)")
                        .font(.caption)
                        .foregroundStyle(unresolvedDelta == 0 ? Color.secondary : Color.orange)
                }
                if let coverageDelta = c.proposalLoopComparison.coverageDelta {
                    Text("Coverage addressed delta: \(coverageDelta > 0 ? "+" : "")\(coverageDelta)")
                        .font(.caption)
                        .foregroundStyle(coverageDelta == 0 ? Color.secondary : Color.blue)
                }
                if let growthA = c.proposalLoopComparison.proposalGrowthRatioA,
                   let growthB = c.proposalLoopComparison.proposalGrowthRatioB {
                    Text(String(format: "Proposal growth ratio: A %.2fx vs B %.2fx", growthA, growthB))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if let scoreDeltaA = c.proposalLoopComparison.scoreDeltaA,
                   let scoreDeltaB = c.proposalLoopComparison.scoreDeltaB {
                    Text(String(format: "Score delta: A %.2f vs B %.2f", scoreDeltaA, scoreDeltaB))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if let rationaleA = c.proposalLoopComparison.targetedRereviewRationaleA {
                    Text("Run A targeted rereview: \(rationaleA)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                if let rationaleB = c.proposalLoopComparison.targetedRereviewRationaleB {
                    Text("Run B targeted rereview: \(rationaleB)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Text(c.proposalLoopComparison.rationale)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func bindingSummary(_ binding: RunComparison.AgentBinding?) -> String {
        guard let binding else { return "—" }
        var parts = [binding.provider]
        if let model = binding.model { parts.append(model) }
        parts.append(binding.effort)
        // Proposal 011 (REQ-009): Show provenance source from frozen data.
        if let source = binding.provenanceSource {
            parts.append("[\(source)]")
        }
        return parts.joined(separator: " / ")
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

    @ViewBuilder
    private func comparisonNumberRow(_ label: String, valueA: String, valueB: String) -> some View {
        HStack {
            Text(label)
                .frame(width: 100, alignment: .leading)
                .font(.caption2)
            Text(valueA)
                .foregroundStyle(.blue)
                .frame(width: 70, alignment: .trailing)
            Text("vs")
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text(valueB)
                .foregroundStyle(.purple)
                .frame(width: 70, alignment: .trailing)
                .font(.caption2)
        }
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

    private func strategyColor(_ status: StrategyRecommendationStatus) -> Color {
        switch status {
        case .candidateWinner:
            return .green
        case .insufficientEvidence:
            return .orange
        case .notEvaluated, .inconclusive:
            return .secondary
        }
    }
}
