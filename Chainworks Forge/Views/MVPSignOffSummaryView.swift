import SwiftUI
import SwiftData

// MARK: - MVPSignOffSummaryView (Proposal 008 — §7.4, §5.7)

/// Shell-owned subroute under RunReportView / export flow.
/// NOT a parallel top-level destination.
///
/// Displays the complete MVP sign-off decision with:
/// - Cohort identity and member list
/// - Pair identity
/// - Manual vs app comparison table
/// - Checkpoint timings (proposal, implementation, release, total)
/// - Median calculation (inputs + outputs)
/// - GO/HOLD result with visual treatment
/// - Explicit failing gate reasons when HOLD
/// - Evaluator version and payload checksum
/// - Link to exported sign-off packet
struct MVPSignOffSummaryView: View {
    let run: Run
    @Environment(\.modelContext) private var modelContext
    @State private var snapshot: MVPSignOffDecisionSnapshot?
    @State private var cohort: BenchmarkCohort?
    @State private var pairs: [BenchmarkPair] = []
    @State private var exportMessage: String?
    @State private var isExporting = false
    @State private var dataLoadWarning: String?

    var body: some View {
        ScrollView {
            if let snapshot {
                VStack(alignment: .leading, spacing: 16) {
                    if let dataLoadWarning {
                        Label(dataLoadWarning, systemImage: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }

                    // 1. Decision header -- GO/HOLD dominant
                    decisionHeader

                    Divider()

                    // 2. Cohort identity and member list
                    cohortIdentitySection

                    // 3. Pair comparison table
                    if !pairs.isEmpty {
                        pairComparisonSection
                    }

                    // 4. Checkpoint timings
                    checkpointTimingsSection

                    // 5. Median calculation
                    medianCalculationSection

                    // 6. Failing gate reasons (only for HOLD)
                    if snapshot.decision == .hold {
                        failingGatesSection
                    }

                    Divider()

                    // 7. Evaluator metadata
                    evaluatorMetadataSection

                    // 8. Export sign-off packet
                    exportSection

                    if let exportMessage {
                        Text(exportMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .transition(.opacity)
                    }
                }
                .padding()
            } else {
                VStack(spacing: 12) {
                    if let dataLoadWarning {
                        Label(dataLoadWarning, systemImage: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }
                    ContentUnavailableView(
                        "No Sign-Off Evaluation",
                        systemImage: "checkmark.seal",
                        description: Text("No MVP sign-off evaluation has been computed for this run's benchmark cohort yet.")
                    )
                }
            }
        }
        .navigationTitle("MVP Sign-Off")
        .task {
            loadSignOffData()
            loadCohortData()
        }
    }

    // MARK: - Decision Header (§5.7 — GO/HOLD visual treatment)

    private var decisionHeader: some View {
        GroupBox {
            VStack(spacing: 12) {
                // GO/HOLD dominant badge
                HStack {
                    Spacer()
                    VStack(spacing: 6) {
                        Image(systemName: decisionIcon)
                            .font(.system(size: 48))
                            .foregroundStyle(decisionColor)
                        Text(snapshot!.decision == .go ? "GO" : "HOLD")
                            .font(.system(size: 36, weight: .heavy, design: .rounded))
                            .foregroundStyle(decisionColor)
                        Text("MVP Sign-Off Decision")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                }
                .padding(.vertical, 8)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(decisionColor.opacity(0.08))
                )

                // Summary stats
                HStack(spacing: 0) {
                    statCell(label: "Pairs", value: "\(snapshot!.pairCount)")
                    Divider().frame(height: 36)
                    statCell(label: "Happy Path", value: "\(snapshot!.happyPathCount)")
                    Divider().frame(height: 36)
                    statCell(label: "Recovered", value: "\(snapshot!.recoveredCount)")
                    Divider().frame(height: 36)
                    statCell(
                        label: "Failing Gates",
                        value: "\(snapshot!.failingGateReasons.count)"
                    )
                }
            }
        } label: {
            Label("Decision", systemImage: decisionIcon)
                .foregroundStyle(decisionColor)
        }
    }

    private func statCell(label: String, value: String) -> some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.title2.monospaced().bold())
            Text(label)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Cohort Identity (§5.7)

    @ViewBuilder
    private var cohortIdentitySection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                if let cohort {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(cohort.label)
                                .font(.headline)
                            Text("Cohort ID: \(cohort.id.uuidString.prefix(8))")
                                .font(.caption.monospaced())
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text(cohort.status.rawValue.capitalized)
                            .font(.caption.bold())
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(cohortStatusColor.opacity(0.15))
                            .foregroundStyle(cohortStatusColor)
                            .clipShape(Capsule())
                    }

                    // Member list
                    let members = cohort.ideaMembers
                    if !members.isEmpty {
                        Divider()
                        Text("Members (\(members.count))")
                            .font(.caption.bold())
                            .foregroundStyle(.secondary)
                        ForEach(members, id: \.ideaIdentifier) { member in
                            HStack(spacing: 8) {
                                Image(systemName: "lightbulb.fill")
                                    .font(.caption2)
                                    .foregroundStyle(.yellow)
                                Text(member.title)
                                    .font(.caption)
                                Spacer()
                                Text(member.repositoryID)
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }

                    // Repository profiles
                    let profiles = cohort.repositoryProfiles
                    if !profiles.isEmpty {
                        Divider()
                        Text("Repository Profiles (\(profiles.count))")
                            .font(.caption.bold())
                            .foregroundStyle(.secondary)
                        ForEach(profiles, id: \.repositoryID) { profile in
                            HStack(spacing: 8) {
                                Image(systemName: "arrow.triangle.branch")
                                    .font(.caption2)
                                    .foregroundStyle(.indigo)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(profile.profileName)
                                        .font(.caption.bold())
                                    Text(profile.description)
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Text(profile.repositoryID)
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.tertiary)
                            }
                        }
                    }
                } else {
                    HStack {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.secondary)
                        Text("Cohort data not available (ID: \(snapshot!.cohortID.uuidString.prefix(8)))")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        } label: {
            Label("Cohort Identity", systemImage: "person.3.fill")
        }
    }

    // MARK: - Pair Comparison Table (§5.7 — Manual vs App)

    private var pairComparisonSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                // Table header
                HStack(spacing: 0) {
                    Text("Idea")
                        .font(.caption.bold())
                        .frame(width: 140, alignment: .leading)
                    Text("Manual")
                        .font(.caption.bold())
                        .foregroundStyle(.orange)
                        .frame(width: 100, alignment: .trailing)
                    Text("App")
                        .font(.caption.bold())
                        .foregroundStyle(.blue)
                        .frame(width: 100, alignment: .trailing)
                    Text("Outcome (M)")
                        .font(.caption.bold())
                        .frame(width: 100, alignment: .center)
                    Text("Outcome (A)")
                        .font(.caption.bold())
                        .frame(width: 100, alignment: .center)
                }
                .padding(.bottom, 4)

                Divider()

                ForEach(pairs) { pair in
                    HStack(spacing: 0) {
                        Text(pair.ideaIdentifier)
                            .font(.caption.monospaced())
                            .lineLimit(1)
                            .frame(width: 140, alignment: .leading)

                        // Manual timing
                        Text(formatOptionalDuration(pair.manualRecord?.totalOrchestrationTimeSeconds))
                            .font(.caption.monospaced())
                            .foregroundStyle(.orange)
                            .frame(width: 100, alignment: .trailing)

                        // App timing
                        Text(formatOptionalDuration(pair.appDrivenRecord?.totalOrchestrationTimeSeconds))
                            .font(.caption.monospaced())
                            .foregroundStyle(.blue)
                            .frame(width: 100, alignment: .trailing)

                        // Manual outcome
                        outcomeBadge(pair.manualRecord?.terminalOutcome)
                            .frame(width: 100)

                        // App outcome
                        outcomeBadge(pair.appDrivenRecord?.terminalOutcome)
                            .frame(width: 100)
                    }
                    .padding(.vertical, 2)
                }
            }
        } label: {
            Label("Pair Comparison", systemImage: "arrow.left.arrow.right")
        }
    }

    private func outcomeBadge(_ outcome: BenchmarkExecutionOutcome?) -> some View {
        let label: String
        let color: Color

        switch outcome {
        case .happyPathCompleted:
            label = "Happy"
            color = .green
        case .recoveredNonHappyPathCompleted:
            label = "Recovered"
            color = .orange
        case .failedUnrecovered:
            label = "Failed"
            color = .red
        case .pending:
            label = "Pending"
            color = .secondary
        case nil:
            label = "--"
            color = .secondary
        }

        return Text(label)
            .font(.caption2.bold())
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
    }

    // MARK: - Checkpoint Timings (§5.7)

    private var checkpointTimingsSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text("Checkpoint timings are median values across all pairs in the cohort.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                HStack(spacing: 0) {
                    Text("Checkpoint")
                        .font(.caption.bold())
                        .frame(width: 180, alignment: .leading)
                    Text("Median")
                        .font(.caption.bold())
                        .frame(width: 100, alignment: .trailing)
                }
                .padding(.bottom, 2)

                Divider()

                timingRow(
                    label: "Proposal Approval",
                    icon: "doc.text.fill",
                    seconds: snapshot!.medianProposalApprovalSeconds
                )
                timingRow(
                    label: "Implementation Approval",
                    icon: "hammer.fill",
                    seconds: snapshot!.medianImplementationApprovalSeconds
                )
                timingRow(
                    label: "Release Decision",
                    icon: "shippingbox.fill",
                    seconds: snapshot!.medianReleaseDecisionSeconds
                )

                Divider()

                // Total orchestration
                HStack(spacing: 0) {
                    HStack(spacing: 6) {
                        Image(systemName: "clock.fill")
                            .font(.caption)
                            .foregroundStyle(.blue)
                        Text("Manual Orchestration")
                            .font(.caption.bold())
                    }
                    .frame(width: 180, alignment: .leading)
                    Text(formatOptionalDuration(snapshot!.medianManualOrchestrationSeconds))
                        .font(.caption.monospaced().bold())
                        .foregroundStyle(.orange)
                        .frame(width: 100, alignment: .trailing)
                }

                HStack(spacing: 0) {
                    HStack(spacing: 6) {
                        Image(systemName: "clock.fill")
                            .font(.caption)
                            .foregroundStyle(.blue)
                        Text("App Orchestration")
                            .font(.caption.bold())
                    }
                    .frame(width: 180, alignment: .leading)
                    Text(formatOptionalDuration(snapshot!.medianAppOrchestrationSeconds))
                        .font(.caption.monospaced().bold())
                        .foregroundStyle(.blue)
                        .frame(width: 100, alignment: .trailing)
                }
            }
        } label: {
            Label("Checkpoint Timings", systemImage: "timer")
        }
    }

    private func timingRow(label: String, icon: String, seconds: Double?) -> some View {
        HStack(spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(label)
                    .font(.caption)
            }
            .frame(width: 180, alignment: .leading)
            Text(formatOptionalDuration(seconds))
                .font(.caption.monospaced())
                .frame(width: 100, alignment: .trailing)
        }
    }

    // MARK: - Median Calculation (§5.7 — Inputs + Outputs)

    private var medianCalculationSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text("Median calculation shows inputs (per-pair timings) and the derived output.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                // Inputs
                DisclosureGroup {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(pairs) { pair in
                            HStack {
                                Text(pair.ideaIdentifier)
                                    .font(.caption2.monospaced())
                                    .frame(width: 120, alignment: .leading)
                                Text("M: \(formatOptionalDuration(pair.manualRecord?.totalOrchestrationTimeSeconds))")
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.orange)
                                    .frame(width: 80, alignment: .trailing)
                                Text("A: \(formatOptionalDuration(pair.appDrivenRecord?.totalOrchestrationTimeSeconds))")
                                    .font(.caption2.monospaced())
                                    .foregroundStyle(.blue)
                                    .frame(width: 80, alignment: .trailing)
                            }
                        }
                    }
                } label: {
                    Label("Inputs (Per-Pair Timings)", systemImage: "list.bullet")
                        .font(.caption.bold())
                }

                Divider()

                // Outputs
                VStack(alignment: .leading, spacing: 6) {
                    Text("Outputs")
                        .font(.caption.bold())
                        .foregroundStyle(.secondary)

                    HStack(spacing: 16) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Median Manual")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(formatOptionalDuration(snapshot!.medianManualOrchestrationSeconds))
                                .font(.headline.monospaced())
                                .foregroundStyle(.orange)
                        }

                        Image(systemName: "arrow.right")
                            .foregroundStyle(.secondary)

                        VStack(alignment: .leading, spacing: 2) {
                            Text("Median App")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(formatOptionalDuration(snapshot!.medianAppOrchestrationSeconds))
                                .font(.headline.monospaced())
                                .foregroundStyle(.blue)
                        }

                        Image(systemName: "equal")
                            .foregroundStyle(.secondary)

                        VStack(alignment: .leading, spacing: 2) {
                            Text("Improvement")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            if let improvement = snapshot!.medianImprovementPercent {
                                Text(String(format: "%+.1f%%", improvement))
                                    .font(.headline.monospaced().bold())
                                    .foregroundStyle(improvement >= 0 ? .green : .red)
                            } else {
                                Text("--")
                                    .font(.headline.monospaced())
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
        } label: {
            Label("Median Calculation", systemImage: "function")
        }
    }

    // MARK: - Failing Gate Reasons (§5.7 — HOLD only)

    private var failingGatesSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                    Text("The following gates prevented a GO decision:")
                        .font(.caption.bold())
                        .foregroundStyle(.red)
                }

                ForEach(Array(snapshot!.failingGateReasons.enumerated()), id: \.offset) { index, reason in
                    HStack(alignment: .top, spacing: 8) {
                        Text("\(index + 1).")
                            .font(.caption.monospaced().bold())
                            .foregroundStyle(.red)
                            .frame(width: 24, alignment: .trailing)
                        Text(reason)
                            .font(.caption)
                            .textSelection(.enabled)
                    }
                    .padding(.vertical, 2)
                }
            }
        } label: {
            Label("Failing Gates (\(snapshot!.failingGateReasons.count))", systemImage: "xmark.shield.fill")
                .foregroundStyle(.red)
        }
    }

    // MARK: - Evaluator Metadata

    private var evaluatorMetadataSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 16) {
                    LabeledContent("Evaluator Version") {
                        Text(snapshot!.evaluatorVersion)
                            .font(.caption.monospaced())
                    }
                    LabeledContent("Evaluated At") {
                        Text(snapshot!.evaluatedAt.formatted(.dateTime))
                            .font(.caption)
                    }
                }

                HStack(spacing: 16) {
                    LabeledContent("Payload Checksum") {
                        Text(snapshot!.payloadChecksum)
                            .font(.caption.monospaced())
                            .textSelection(.enabled)
                            .lineLimit(1)
                    }
                    LabeledContent("Snapshot ID") {
                        Text(snapshot!.id.uuidString.prefix(8))
                            .font(.caption.monospaced())
                    }
                }

                LabeledContent("Cohort ID") {
                    Text(snapshot!.cohortID.uuidString)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                        .lineLimit(1)
                }
            }
            .font(.caption)
        } label: {
            Label("Evaluator Metadata", systemImage: "info.circle")
        }
    }

    // MARK: - Export Section

    private var exportSection: some View {
        GroupBox {
            HStack(spacing: 12) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.title3)
                    .foregroundStyle(decisionColor)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Export Sign-Off Packet")
                        .font(.subheadline.bold())
                    Text("Decision snapshot, payload, median calculations, and gate evaluation results.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button {
                    exportSignOffPacket()
                } label: {
                    Label("Export", systemImage: "square.and.arrow.up")
                }
                .buttonStyle(.borderedProminent)
                .tint(decisionColor)
                .disabled(isExporting)
            }
        } label: {
            Label("Export", systemImage: "square.and.arrow.up")
        }
    }

    // MARK: - Data Loading

    /// Look up the sign-off snapshot from the run's linked benchmark pair/cohort.
    private func loadSignOffData() {
        dataLoadWarning = nil
        // Find a benchmark pair that links to this run
        let allPairsDescriptor = FetchDescriptor<BenchmarkPair>()
        let allPairs: [BenchmarkPair]
        do {
            allPairs = try modelContext.fetch(allPairsDescriptor)
        } catch {
            let message = "Failed to load benchmark pairs: \(error.localizedDescription)"
            dataLoadWarning = message
            ForgeLogger.ui.error("MVPSignOffSummaryView failed to fetch benchmark pairs for run \(run.id): \(error.localizedDescription)")
            return
        }
        guard let pair = allPairs.first(where: { $0.appDrivenRecord?.linkedRunID == run.id }) else { return }
        guard let cohortID = pair.cohort?.id else { return }

        // Find the latest snapshot for this cohort
        let snapshotDescriptor = FetchDescriptor<MVPSignOffDecisionSnapshot>(
            sortBy: [SortDescriptor(\.evaluatedAt, order: .reverse)]
        )
        let allSnapshots: [MVPSignOffDecisionSnapshot]
        do {
            allSnapshots = try modelContext.fetch(snapshotDescriptor)
        } catch {
            let message = "Failed to load sign-off snapshots: \(error.localizedDescription)"
            dataLoadWarning = message
            ForgeLogger.ui.error("MVPSignOffSummaryView failed to fetch sign-off snapshots for run \(run.id): \(error.localizedDescription)")
            return
        }
        snapshot = allSnapshots.first(where: { $0.cohortID == cohortID })
    }

    private func loadCohortData() {
        guard let snapshot else { return }
        // Load cohort
        let cohortID = snapshot.cohortID
        let cohortDescriptor = FetchDescriptor<BenchmarkCohort>(
            predicate: #Predicate<BenchmarkCohort> { cohort in
                cohort.id == cohortID
            }
        )
        do {
            cohort = try modelContext.fetch(cohortDescriptor).first
        } catch {
            let message = "Failed to load benchmark cohort: \(error.localizedDescription)"
            dataLoadWarning = dataLoadWarning ?? message
            ForgeLogger.ui.error("MVPSignOffSummaryView failed to fetch benchmark cohort for run \(run.id): \(error.localizedDescription)")
            cohort = nil
        }

        // Load pairs
        if let cohort {
            pairs = cohort.pairs.sorted { $0.createdAt < $1.createdAt }
        }
    }

    // MARK: - Export

    /// Proposal 008 (REQ-008): Export replayable sign-off packet using the dedicated builder.
    private func exportSignOffPacket() {
        isExporting = true
        exportMessage = nil

        guard let snapshot, let cohort else {
            exportMessage = "Cannot export: missing snapshot or cohort data."
            isExporting = false
            return
        }

        let desktopURL = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory

        do {
            let builder = SignOffEvidencePackBuilder(modelContext: modelContext)
            let packet = try builder.buildCohortPacket(cohort: cohort, snapshot: snapshot)
            let exportDir = desktopURL.appendingPathComponent(
                "signoff-packet-\(snapshot.id.uuidString.prefix(8))",
                isDirectory: true
            )
            try builder.exportToFile(packet: packet, destinationURL: exportDir)
            exportMessage = "Sign-off packet exported to Desktop via SignOffEvidencePackBuilder."
        } catch {
            exportMessage = "Export failed: \(error.localizedDescription)"
        }

        isExporting = false
    }

    // MARK: - Computed Properties

    private var decisionIcon: String {
        snapshot!.decision == .go ? "checkmark.seal.fill" : "hand.raised.fill"
    }

    private var decisionColor: Color {
        snapshot!.decision == .go ? .green : .red
    }

    private var cohortStatusColor: Color {
        guard let cohort else { return .secondary }
        switch cohort.status {
        case .active: return .blue
        case .completed: return .green
        case .superseded: return .gray
        }
    }

    // MARK: - Formatting Helpers

    private func formatOptionalDuration(_ seconds: Double?) -> String {
        guard let seconds else { return "--" }
        let totalSeconds = Int(seconds)
        let mins = totalSeconds / 60
        let secs = totalSeconds % 60
        if mins >= 60 {
            let hrs = mins / 60
            let remainMins = mins % 60
            return "\(hrs)h \(remainMins)m"
        }
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }
}
