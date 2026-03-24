import SwiftUI

struct PreflightReportView: View {
    let report: PreflightReport

    var body: some View {
        List {
            Section("Summary") {
                LabeledContent("Status", value: report.status.rawValue.capitalized)
                LabeledContent("Configuration Source", value: report.configurationSource.displayName)
            }

            Section("Checks") {
                ForEach(report.checks) { check in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(check.title)
                            Spacer()
                            Text(check.status.rawValue.uppercased())
                                .font(.caption2.bold())
                                .foregroundStyle(color(for: check.status))
                        }
                        Text(check.category)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text(check.message)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 2)
                }
            }

            if !report.warnings.isEmpty {
                Section("Warnings") {
                    ForEach(report.warnings, id: \.self) { warning in
                        Text(warning)
                    }
                }
            }

            if !report.blockingIssues.isEmpty {
                Section("Blocking Issues") {
                    ForEach(report.blockingIssues, id: \.self) { issue in
                        Text(issue)
                            .foregroundStyle(.red)
                    }
                }
            }
        }
        .navigationTitle("Preflight Report")
        .accessibilityIdentifier("preflight-report-view")
    }

    private func color(for status: PreflightCheckStatus) -> Color {
        switch status {
        case .pass:
            return .green
        case .warn:
            return .orange
        case .fail:
            return .red
        }
    }
}
