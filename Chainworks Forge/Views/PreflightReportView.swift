import SwiftUI

struct PreflightReportView: View {
    let report: PreflightReport

    var body: some View {
        List {
            Section("Summary") {
                LabeledContent("Status", value: report.status.rawValue.capitalized)
                LabeledContent("Configuration Source", value: report.configurationSource.displayName)
            }

            if let rollout = report.rolloutDecisionSummary {
                Section("Rollout") {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Image(systemName: icon(for: rollout.backendDecision))
                                .foregroundStyle(color(for: rollout.backendDecision))
                            Text(rollout.backendDecision.replacingOccurrences(of: "_", with: " ").capitalized)
                                .font(.headline)
                            Spacer()
                            Text(rollout.status.uppercased())
                                .font(.caption2.bold())
                                .foregroundStyle(color(for: rollout.backendDecision))
                        }

                        LabeledContent("Enforcement", value: rollout.enforcementMode)
                        LabeledContent("Projection", value: rollout.projectionIntegrity)
                        LabeledContent("Source", value: rollout.sourceLane)
                        if rollout.waiverState != "none" {
                            LabeledContent("Waiver", value: rollout.waiverState)
                        }
                        if let expiresAt = rollout.waiverExpiresAt {
                            LabeledContent("Waiver Expires", value: expiresAt)
                        }
                        if let cutover = rollout.cutoverPolicyRevision {
                            LabeledContent("Cutover", value: cutover)
                        }
                        Text(rollout.operatorMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 2)

                    if !rollout.holdConditions.isEmpty {
                        rolloutList("Hold Conditions", rollout.holdConditions, color: .red)
                    }

                    if rollout.rollbackDisposition.mode != "not_applicable"
                        || !rollout.rollbackDisposition.steps.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Rollback")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text("\(rollout.rollbackDisposition.mode) / \(rollout.rollbackDisposition.dataLossRisk)")
                                .font(.caption)
                            ForEach(rollout.rollbackDisposition.steps, id: \.self) { step in
                                Text(step)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding(.vertical, 2)
                    }

                    if !rollout.nextSteps.isEmpty {
                        rolloutList("Next Steps", rollout.nextSteps, color: .secondary)
                    }
                }
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

    private func rolloutList(_ title: String, _ values: [String], color: Color) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
            ForEach(values, id: \.self) { value in
                Text(value)
                    .font(.caption)
                    .foregroundStyle(color)
            }
        }
        .padding(.vertical, 2)
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

    private func color(for backendDecision: String) -> Color {
        switch backendDecision {
        case "release", "waive", "not_applicable":
            return .green
        case "hold", "rollback_required":
            return .red
        default:
            return .orange
        }
    }

    private func icon(for backendDecision: String) -> String {
        switch backendDecision {
        case "release":
            return "checkmark.shield.fill"
        case "waive":
            return "checkmark.seal.fill"
        case "not_applicable":
            return "minus.circle.fill"
        case "hold", "rollback_required":
            return "exclamationmark.triangle.fill"
        default:
            return "questionmark.circle.fill"
        }
    }
}
