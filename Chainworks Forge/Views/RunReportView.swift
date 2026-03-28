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
    @Environment(\.modelContext) private var modelContext
    @State private var selectedTab: ReportTab = .latestSummary
    @State private var reportArtifacts: [Artifact] = []
    @State private var summaryContent: String?
    @State private var selectedReportContent: String?
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
        }
    }

    private var trustLabel: String {
        switch selectedTab {
        case .latestSummary: return "Mutable latest summary — may change on recovery"
        case .immutableHistory: return "Immutable history — never overwritten"
        case .exportHub: return "Export hub — completed run evidence and receipts"
        case .signOffSummary: return "Sign-off summary — benchmark evaluation"
        }
    }

    private var trustColor: Color {
        switch selectedTab {
        case .latestSummary: return .orange
        case .immutableHistory: return .green
        case .exportHub: return .blue
        case .signOffSummary: return .purple
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
            Text(content)
                .font(.body.monospaced())
                .textSelection(.enabled)
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
                    Text(content)
                        .font(.body.monospaced())
                        .textSelection(.enabled)
                        .padding()
                }
            }
            .padding()
        }
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

        // Proposal 008 (PERF-080): Measure summary retrieval latency via SLO probe.
        if let summaryID = run.latestSummaryArtifactID {
            let summaryDescriptor = FetchDescriptor<Artifact>(
                predicate: #Predicate<Artifact> { artifact in
                    artifact.id == summaryID
                }
            )
            if let summaryArtifact = try? modelContext.fetch(summaryDescriptor).first {
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

        isLoadingReport = false
    }

    /// Proposal 008 (REQ-012): Retry report retrieval after error or timeout.
    private func retryLoadReport() {
        summaryContent = nil
        selectedReportContent = nil
        loadReportData()
    }

    private func loadReportContent(_ artifact: Artifact) {
        // Proposal 008 (PERF-080): Measure report retrieval latency.
        selectedReportContent = try? sloProbe.measure(
            artifactName: artifact.name,
            runID: run.id
        ) {
            try String(contentsOfFile: artifact.filePath, encoding: .utf8)
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
