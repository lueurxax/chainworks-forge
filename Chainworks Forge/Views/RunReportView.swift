import SwiftUI
import SwiftData

// MARK: - P005-OPS §6: Run Report View

/// Displays immutable run reports and latest summary.
/// Clearly labels whether operator is reading immutable history or latest summary (§6.2).
struct RunReportView: View {
    let run: Run
    @Environment(\.modelContext) private var modelContext
    @State private var selectedTab: ReportTab = .latestSummary
    @State private var reportArtifacts: [Artifact] = []
    @State private var summaryContent: String?
    @State private var selectedReportContent: String?

    enum ReportTab: String, CaseIterable {
        case latestSummary = "Latest Summary"
        case immutableHistory = "Immutable History"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Tab picker
            Picker("Report View", selection: $selectedTab) {
                ForEach(ReportTab.allCases, id: \.self) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .padding()

            Divider()

            // Trust label
            HStack {
                Image(systemName: selectedTab == .latestSummary ? "arrow.clockwise" : "lock.fill")
                Text(selectedTab == .latestSummary
                     ? "Mutable latest summary — may change on recovery"
                     : "Immutable history — never overwritten")
                    .font(.caption)
                    .foregroundStyle(selectedTab == .latestSummary ? .orange : .green)
                Spacer()
                RuntimeProvenanceBadge(trustLevel: run.runtimeTrustLevel)
            }
            .padding(.horizontal)
            .padding(.vertical, 4)
            .background(selectedTab == .latestSummary ? Color.orange.opacity(0.05) : Color.green.opacity(0.05))

            Divider()

            // Content
            ScrollView {
                switch selectedTab {
                case .latestSummary:
                    if let content = summaryContent {
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

                case .immutableHistory:
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
            }
        }
        .navigationTitle("Run Report")
        .task {
            loadReportData()
        }
    }

    private func loadReportData() {
        // Load immutable report artifacts
        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { artifact in
                artifact.runID == runID && artifact.reportKind == "immutable_history"
            },
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        reportArtifacts = (try? modelContext.fetch(descriptor)) ?? []

        // Load latest summary
        if let summaryID = run.latestSummaryArtifactID {
            let summaryDescriptor = FetchDescriptor<Artifact>(
                predicate: #Predicate<Artifact> { artifact in
                    artifact.id == summaryID
                }
            )
            if let summaryArtifact = try? modelContext.fetch(summaryDescriptor).first {
                summaryContent = try? String(contentsOfFile: summaryArtifact.filePath, encoding: .utf8)
            }
        }
    }

    private func loadReportContent(_ artifact: Artifact) {
        selectedReportContent = try? String(contentsOfFile: artifact.filePath, encoding: .utf8)
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
