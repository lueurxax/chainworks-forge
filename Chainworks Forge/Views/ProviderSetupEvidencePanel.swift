import SwiftUI

struct ProviderSetupEvidencePanel: View {
    let snapshot: GooseProviderAssistantSnapshot

    private var report: ProviderTroubleshootingReport? { snapshot.report }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Evidence Panel")
                    .font(.headline)
                Spacer()
                Text((report?.status.displayName) ?? snapshot.journeyState.displayName)
                    .font(.caption.weight(.semibold))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(statusColor.opacity(0.15), in: Capsule())
                    .foregroundStyle(statusColor)
            }

            LabeledContent("Family", value: snapshot.family.displayName)
                .font(.caption)
            LabeledContent("Transport", value: snapshot.transport.displayName)
                .font(.caption)
            LabeledContent("Auth Mode", value: snapshot.authMode.displayName)
                .font(.caption)
            LabeledContent("Endpoint", value: snapshot.endpoint ?? "Not configured")
                .font(.caption)
            LabeledContent("Goose Provider", value: snapshot.providerIdentifier)
                .font(.caption)
            LabeledContent("Selected / Default Model", value: snapshot.configuredModel ?? "Runtime default")
                .font(.caption)
            LabeledContent("Latest Verification Result", value: snapshot.journeyState.displayName)
                .font(.caption)
            LabeledContent("Latest Checked", value: checkedAtText)
                .font(.caption)

            if let report {
                LabeledContent("Failure Layer", value: report.failureLayer.displayName)
                    .font(.caption)
            }

            if !snapshot.availableModels.isEmpty {
                LabeledContent("Available Models", value: snapshot.availableModels.joined(separator: ", "))
                    .font(.caption)
            }

            if !snapshot.handshakeSteps.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Handshake Steps")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(snapshot.handshakeSteps) { step in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(step.label)
                                    .font(.caption.weight(.semibold))
                                Spacer()
                                Text(step.state.displayName)
                                    .font(.caption2.weight(.semibold))
                                    .foregroundStyle(stepColor(for: step.state))
                            }
                            Text(step.value)
                                .font(.caption)
                            if let detail = step.detail {
                                Text(detail)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }

            if let report, !report.evidence.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Handshake Facts")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(report.evidence) { item in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.label)
                                .font(.caption.weight(.semibold))
                            Text(item.value)
                                .font(.caption)
                                .foregroundStyle(styleColor(for: item.state))
                        }
                    }
                }
            }

            if let report, !report.remediation.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Recommended Next Actions")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(report.remediation, id: \.self) { item in
                        Label(item, systemImage: "arrow.right.circle")
                            .font(.caption)
                    }
                }
            }

            if let report {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Raw Probe Details")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(rawDetails(for: report))
                        .font(.caption2.monospaced())
                        .textSelection(.enabled)
                }
            }
        }
        .padding(12)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(statusColor.opacity(0.2), lineWidth: 1)
        )
        .accessibilityIdentifier("provider-setup-evidence-panel")
    }

    private var statusColor: Color {
        switch report?.status {
        case .none:
            return stepColor(for: snapshot.journeyState == .failing ? .failed : snapshot.journeyState == .degraded ? .warning : .passed)
        case .healthy:
            return .green
        case .warning:
            return .orange
        case .blocked:
            return .red
        }
    }

    private var checkedAtText: String {
        guard let checkedAt = snapshot.checkedAt else { return "Not checked yet" }
        return checkedAt.formatted(date: .abbreviated, time: .shortened)
    }

    private func styleColor(for state: ProviderTroubleshootingEvidenceState) -> Color {
        switch state {
        case .info:
            return .secondary
        case .warning:
            return .orange
        case .blocked:
            return .red
        }
    }

    private func stepColor(for state: GooseHandshakeStepState) -> Color {
        switch state {
        case .pending:
            return .secondary
        case .passed:
            return .green
        case .warning:
            return .orange
        case .failed:
            return .red
        }
    }

    private func rawDetails(for report: ProviderTroubleshootingReport) -> String {
        let payload: [String: Any] = [
            "providerID": report.providerID.uuidString,
            "displayName": report.displayName,
            "transport": report.transport.rawValue,
            "status": report.status.rawValue,
            "failureLayer": report.failureLayer.rawValue,
            "availableModels": report.availableModels,
            "evidence": report.evidence.map { ["label": $0.label, "value": $0.value, "state": $0.state.rawValue] }
        ]

        guard
            let data = try? JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted, .sortedKeys]),
            let text = String(data: data, encoding: .utf8)
        else {
            return "Raw probe details unavailable."
        }
        return text
    }
}
