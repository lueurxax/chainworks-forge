import SwiftUI
import SwiftData

// MARK: - P005-OPS §6 + Proposal 008 §7.3–7.4: Run Report View

/// Displays immutable run reports, latest summary, completed-run export hub,
/// and MVP sign-off summary.
///
/// Proposal 008 additions:
/// - Export Hub tab (§7.3): dominant summary, receipts, evidence-pack status, export actions
/// - Sign-Off Summary tab (§7.4): cohort/pair comparison, GO/HOLD result
struct RunReportView: View {
    let run: Run
    private let autoSelectLatestImmutableReport: Bool
    @Environment(\.modelContext) private var modelContext
    @State private var selectedTab: ReportTab = .latestSummary
    @State private var reportArtifacts: [Artifact] = []
    @State private var summaryContent: String?
    @State private var summaryArtifact: Artifact?
    @State private var selectedReportContent: String?
    @State private var selectedReportArtifact: Artifact?
    @State private var strategyRecommendation: StrategyRecommendation?
    @State private var strategyPairComparison: RunComparison?
    // Proposal 008 (REQ-012): Loading/timeout/retry states for report surfaces.
    @State private var isLoadingReport = true
    @State private var loadError: String?
    @State private var isTimedOut = false
    /// Proposal 008 (PERF-080): SLO measurement for report opens.
    private let sloProbe = OutputRetrievalSLOProbe()

    enum ReportTab: String, CaseIterable {
        case latestSummary = "Latest Summary"
        case immutableHistory = "Immutable History"
        case exportHub = "Export Hub"
        case signOffSummary = "Sign-Off"
        case strategySummary = "Strategy"
    }

    init(
        run: Run,
        initialTab: ReportTab = .latestSummary,
        autoSelectLatestImmutableReport: Bool = false
    ) {
        self.run = run
        self.autoSelectLatestImmutableReport = autoSelectLatestImmutableReport
        self._selectedTab = State(initialValue: initialTab)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Tab picker
            Picker("Report View", selection: $selectedTab) {
                ForEach(availableTabs, id: \.self) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .padding()

            Divider()

            // Trust label
            HStack {
                Image(systemName: trustIcon)
                Text(trustLabel)
                    .font(.caption)
                    .foregroundStyle(trustColor)
                Spacer()
                RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
            }
            .padding(.horizontal)
            .padding(.vertical, 4)
            .background(trustColor.opacity(0.05))

            HStack {
                ParentIdeaArchiveBadge(title: "Parent idea", idea: run.idea)
                StrategyBadge(
                    profileID: strategyProfileID(for: run),
                    assignmentMode: strategyAssignmentMode(for: run),
                    recommendationState: effectiveStrategyRecommendationState(for: run)
                )
                Spacer()
            }
            .padding(.horizontal)
            .padding(.top, 4)

            Divider()

            // Content
            ScrollView {
                switch selectedTab {
                case .latestSummary:
                    latestSummaryContent

                case .immutableHistory:
                    immutableHistoryContent

                case .exportHub:
                    CompletedRunExportHub(run: run)

                case .signOffSummary:
                    MVPSignOffSummaryView(run: run)
                case .strategySummary:
                    strategySummaryContent
                }
            }
        }
        .navigationTitle("Run Report")
        .accessibilityIdentifier("run-report-view")
        .task {
            loadReportData()
        }
    }

    // MARK: - Tab Availability

    /// Only show Export Hub for completed/failed runs with delivery config.
    /// Only show Sign-Off tab for runs that are part of a benchmark pair.
    private var availableTabs: [ReportTab] {
        var tabs: [ReportTab] = [.latestSummary, .immutableHistory]

        if run.status == .completed || run.status == .failed {
            tabs.append(.exportHub)
            tabs.append(.strategySummary)
        }

        // Sign-off tab visible if run might be linked to a benchmark
        if run.status == .completed || run.status == .failed {
            tabs.append(.signOffSummary)
        }

        return tabs
    }

    // MARK: - Trust Badge

    private var trustIcon: String {
        switch selectedTab {
        case .latestSummary: return "arrow.clockwise"
        case .immutableHistory: return "lock.fill"
        case .exportHub: return "shippingbox"
        case .signOffSummary: return "checkmark.seal"
        case .strategySummary: return "chart.bar"
        }
    }

    private var trustLabel: String {
        switch selectedTab {
        case .latestSummary: return "Mutable latest summary — may change on recovery"
        case .immutableHistory: return "Immutable history — never overwritten"
        case .exportHub: return "Export hub — completed run evidence and receipts"
        case .signOffSummary: return "Sign-off summary — benchmark evaluation"
        case .strategySummary: return "Strategy summary — shell-owned comparison lane"
        }
    }

    private var trustColor: Color {
        switch selectedTab {
        case .latestSummary: return .orange
        case .immutableHistory: return .green
        case .exportHub: return .blue
        case .signOffSummary: return .purple
        case .strategySummary: return .pink
        }
    }

    // MARK: - Content Views

    @ViewBuilder
    private var latestSummaryContent: some View {
        if isLoadingReport {
            ProgressView("Loading report...")
                .frame(maxWidth: .infinity, alignment: .center)
                .padding()
        } else if isTimedOut {
            // Proposal 008 (REQ-012): Explicit timeout state with retry action.
            ContentUnavailableView {
                Label("Report Timed Out", systemImage: "clock.badge.exclamationmark")
            } description: {
                Text("Loading exceeded the \(String(format: "%.0f", OutputRetrievalSLOProbe.p95TargetSeconds))s SLO target. The report may still be generating.")
            } actions: {
                Button("Retry") { retryLoadReport() }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("report-retry-button")
            }
        } else if let error = loadError {
            // Proposal 008 (REQ-012): Explicit error state with retry action.
            ContentUnavailableView {
                Label("Load Error", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") { retryLoadReport() }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("report-retry-button")
            }
        } else if let content = summaryContent {
            ArtifactContentRenderer(
                content: content,
                context: summaryArtifact.map { .artifactBacked(artifact: $0, run: run) } ?? .explicit(format: .markdown)
            )
            .padding()
        } else {
            ContentUnavailableView(
                "No Summary",
                systemImage: "doc.text",
                description: Text("No latest summary has been generated yet.")
            )
        }
    }

    @ViewBuilder
    private var immutableHistoryContent: some View {
        if reportArtifacts.isEmpty {
            ContentUnavailableView(
                "No Reports",
                systemImage: "doc.text",
                description: Text("No immutable reports have been emitted yet.")
            )
        } else {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(reportArtifacts.sorted(by: {
                    ($0.reportVersion ?? 0) > ($1.reportVersion ?? 0)
                })) { artifact in
                    ReportVersionRow(artifact: artifact, isSelected: selectedReportContent != nil)
                        .onTapGesture {
                            loadReportContent(artifact)
                        }
                }

                if let content = selectedReportContent {
                    Divider()
                    ArtifactContentRenderer(
                        content: content,
                        context: selectedReportArtifact.map { .artifactBacked(artifact: $0, run: run) } ?? .explicit(format: .report)
                    )
                    .padding()
                }
            }
            .padding()
        }
    }

    @ViewBuilder
    private var strategySummaryContent: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let feedback = proposalLoopSummary {
                GroupBox("Proposal-loop feedback summary (Proposal 022)") {
                    VStack(alignment: .leading, spacing: 4) {
                        let reviewCorpusLabel = feedback.reviewCorpusBundlePresent
                            ? "present (\(feedback.reviewCorpusRawArtifactCount.map(String.init) ?? "unknown") raw)"
                            : "missing"
                        LabeledContent("Review corpus bundle", value: reviewCorpusLabel)
                        LabeledContent("Backlog items", value: "\(feedback.backlogItemCount)")
                        LabeledContent("Unresolved items", value: "\(feedback.unresolvedItemCount)")
                        LabeledContent("Deferred items", value: "\(feedback.deferredItemCount)")
                        LabeledContent("Addressed items", value: "\(feedback.addressedItemCount)")
                        LabeledContent("Merge provenance", value: "\(feedback.mergeProvenanceItemCount)")
                        LabeledContent("Coverage", value: feedback.coverageStatusSummary)
                        if let growthRatio = feedback.proposalGrowthRatio {
                            LabeledContent("Proposal growth", value: String(format: "%.2fx", growthRatio))
                        }
                        if let scoreDelta = feedback.scoreDeltaSinceLastReview {
                            LabeledContent("Score delta", value: String(format: "%.2f", scoreDelta))
                        }
                        if let recommendation = feedback.growthGuardRecommendation {
                            LabeledContent("Growth guard", value: recommendation)
                        }
                        if let nextAction = feedback.boundedNextAction {
                            LabeledContent("Bounded next action", value: nextAction)
                        }
                        if let targeted = feedback.targetedReviewerSummary {
                            Text("Targeted rereview: \(targeted)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }

            GroupBox("Strategy context") {
                VStack(alignment: .leading, spacing: 6) {
                    StrategyBadge(
                        profileID: strategyProfileID(for: run),
                        assignmentMode: strategyAssignmentMode(for: run),
                        recommendationState: effectiveStrategyRecommendationState(for: run)
                    )
                    if let profile = strategyProfileID(for: run) {
                        LabeledContent("Profile", value: profile)
                    } else {
                        LabeledContent("Profile", value: "not captured")
                    }
                    if let mode = strategyAssignmentMode(for: run) {
                        LabeledContent("Assignment mode", value: mode)
                    } else {
                        LabeledContent("Assignment mode", value: "not captured")
                    }
                    if let state = strategyRecommendationState(for: run) {
                        LabeledContent("Recommendation state", value: state)
                    }
                    let promotedArtifacts = promotedHandoffArtifacts(for: run)
                    if !promotedArtifacts.isEmpty {
                        LabeledContent("Promoted handoff artifacts", value: promotedArtifacts.joined(separator: ", "))
                    }
                }
            }

            if let telemetry = strategyTelemetrySummary(for: run) {
                GroupBox("Strategy telemetry") {
                    VStack(alignment: .leading, spacing: 6) {
                        LabeledContent("Payload reduction", value: "\(telemetry.totalPayloadReductionBytes) bytes")
                        LabeledContent("Average cache effectiveness", value: String(format: "%.2f", telemetry.averageCacheEffectiveness))
                        LabeledContent("Compaction churn", value: "\(telemetry.totalCompactionChurn)")
                        LabeledContent("Escalations", value: "\(telemetry.totalEscalationCount)")
                        LabeledContent("Promoted artifact count", value: "\(telemetry.operatorPromotedArtifactCount)")
                    }
                }
            }

            if let summary = strategyPairComparison {
                GroupBox("Comparable run recommendation (shell-owned lane)") {
                    VStack(alignment: .leading, spacing: 6) {
                        if let qualitySummary = summary.strategyComparison.qualityDeltaSummary {
                            Text(qualitySummary)
                                .font(.caption)
                        }
                        Label(
                            "Status: \(summary.strategyRecommendation.status.rawValue)",
                            systemImage: summary.strategyRecommendation.status == .candidateWinner ? "checkmark.circle" : "info.circle"
                        )
                        .foregroundStyle(strategyColor(summary.strategyRecommendation.status))
                        if let recommended = summary.strategyRecommendation.recommendedProfileID {
                            Text("Recommended profile: \(recommended)")
                                .font(.caption)
                        }
                        Text("Proof owner: \(summary.strategyRecommendation.proofOwner)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text("Evaluation set: \(summary.strategyRecommendation.evaluationSetSummary)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        if !summary.strategyRecommendation.holdCriteria.isEmpty {
                            Text("Hold criteria: \(summary.strategyRecommendation.holdCriteria.joined(separator: ", "))")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Text(summary.strategyRecommendation.rationale)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 4)
                }
            } else if let recommendation = strategyRecommendation {
                GroupBox("Strategy recommendation") {
                    VStack(alignment: .leading, spacing: 6) {
                        Label("Status: \(recommendation.status.rawValue)", systemImage: recommendation.status == .candidateWinner ? "checkmark.circle" : "info.circle")
                            .foregroundStyle(strategyColor(recommendation.status))
                        Text("Proof owner: \(recommendation.proofOwner)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text("Evaluation set: \(recommendation.evaluationSetSummary)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        if !recommendation.holdCriteria.isEmpty {
                            Text("Hold criteria: \(recommendation.holdCriteria.joined(separator: ", "))")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Text(recommendation.rationale)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            } else {
                ContentUnavailableView(
                    "No strategy comparison",
                    systemImage: "speedometer",
                    description: Text("A compatible pair with different strategy settings is not available yet.")
                )
            }
        }
        .padding()
    }

    private var proposalLoopSummary: ProposalLoopFeedbackSummary? {
        let artifacts = run.stageExecutions
            .flatMap { $0.agentExecutions }
            .flatMap { $0.artifacts }
        return ProposalLoopFeedbackParser.parseSummary(from: artifacts)
    }

    // MARK: - Data Loading

    private func loadReportData() {
        isLoadingReport = true
        loadError = nil
        isTimedOut = false

        let loadStart = CFAbsoluteTimeGetCurrent()

        // Load immutable report artifacts
        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID && artifact.reportKind == "immutable_history"
            },
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        reportArtifacts = (try? modelContext.fetch(descriptor)) ?? []
        if autoSelectLatestImmutableReport,
           selectedReportContent == nil,
           let latestReport = reportArtifacts.first {
            loadReportContent(latestReport)
        }

        // Proposal 008 (PERF-080): Measure summary retrieval latency via SLO probe.
        if let summaryID = run.latestSummaryArtifactID {
            let summaryDescriptor = FetchDescriptor<Artifact>(
                predicate: #Predicate<Artifact> { artifact in
                    artifact.id == summaryID
                }
            )
            if let summaryArtifact = try? modelContext.fetch(summaryDescriptor).first {
                self.summaryArtifact = summaryArtifact
                do {
                    summaryContent = try sloProbe.measure(
                        artifactName: summaryArtifact.name,
                        runID: run.id
                    ) {
                        try String(contentsOfFile: summaryArtifact.filePath, encoding: .utf8)
                    }
                } catch {
                    loadError = "Failed to load summary: \(error.localizedDescription)"
                }
            }
        }

        // Proposal 008 (REQ-012): Detect timeout when retrieval exceeds SLO target.
        let elapsed = CFAbsoluteTimeGetCurrent() - loadStart
        if elapsed > OutputRetrievalSLOProbe.p95TargetSeconds && summaryContent == nil && loadError == nil {
            isTimedOut = true
        }

        loadStrategyRecommendation()
        isLoadingReport = false
    }

    private func loadStrategyRecommendation() {
        let service = RunComparisonService(modelContext: modelContext)
        let peers = service.compatibleTargets(for: run).sorted { $0.startedAt > $1.startedAt }
        strategyPairComparison = peers.compactMap { peer in
            service.compare(run, peer)
        }.first { candidate in
            candidate.strategyComparison.profileA != candidate.strategyComparison.profileB
        }
        if let comparison = strategyPairComparison {
            strategyRecommendation = comparison.strategyRecommendation
        } else {
            strategyRecommendation = nil
        }
    }

    private func promotedHandoffArtifacts(for run: Run) -> [String] {
        guard
            let data = run.promotedHandoffArtifactsJSON,
            let artifacts = try? JSONDecoder().decode([String].self, from: data)
        else {
            return []
        }
        return artifacts
    }

    private func strategyTelemetrySummary(for run: Run) -> SessionReuseKPIExporter.StrategyTelemetrySummary? {
        guard let data = run.sessionKPIExportJSON else {
            return nil
        }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        guard let summary = try? decoder.decode(SessionReuseKPIExporter.RunKPISummary.self, from: data) else {
            return nil
        }
        return summary.strategyTelemetry
    }

    /// Proposal 008 (REQ-012): Retry report retrieval after error or timeout.
    private func retryLoadReport() {
        summaryContent = nil
        summaryArtifact = nil
        selectedReportContent = nil
        selectedReportArtifact = nil
        loadReportData()
    }

    private func loadReportContent(_ artifact: Artifact) {
        selectedReportArtifact = artifact
        // Proposal 008 (PERF-080): Measure report retrieval latency.
        selectedReportContent = try? sloProbe.measure(
            artifactName: artifact.name,
            runID: run.id
        ) {
            try String(contentsOfFile: artifact.filePath, encoding: .utf8)
        }
    }

    private func strategyProfileID(for run: Run) -> String? {
        let value = run.contextStrategyProfileID.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func strategyAssignmentMode(for run: Run) -> String? {
        let value = run.strategyAssignmentMode.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func strategyRecommendationState(for run: Run) -> String? {
        let value = run.strategyRecommendationState.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func effectiveStrategyRecommendationState(for run: Run) -> String? {
        strategyPairComparison?.strategyRecommendation.status.rawValue
            ?? strategyRecommendation?.status.rawValue
            ?? strategyRecommendationState(for: run)
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

// MARK: - Report Version Row

struct ReportVersionRow: View {
    let artifact: Artifact
    let isSelected: Bool

    var body: some View {
        HStack {
            Image(systemName: "lock.fill")
                .foregroundStyle(.green)
            VStack(alignment: .leading) {
                Text(artifact.name)
                    .font(.headline)
                Text("v\(artifact.reportVersion ?? 0) — \(artifact.createdAt.formatted())")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Image(systemName: "chevron.right")
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
    }
}
