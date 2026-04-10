import SwiftUI

struct ProviderTroubleshootingPanel: View {
    let report: ProviderTroubleshootingReport

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(report.headline)
                        .font(.subheadline.weight(.semibold))
                    Text(report.explanation)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                statusPill
            }

            LabeledContent("Failure Layer", value: report.failureLayer.displayName)
                .font(.caption)

            if !report.evidence.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Evidence")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(report.evidence) { item in
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text(item.label)
                                .font(.caption.weight(.semibold))
                                .frame(width: 150, alignment: .leading)
                            Text(item.value)
                                .font(.caption)
                                .foregroundStyle(styleColor(for: item.state))
                            Spacer(minLength: 0)
                        }
                    }
                }
            }

            if !report.remediation.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Next Actions")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(report.remediation, id: \.self) { step in
                        Label(step, systemImage: "arrow.right.circle")
                            .font(.caption)
                            .foregroundStyle(.primary)
                    }
                }
            }
        }
        .padding(10)
        .background(backgroundColor.opacity(0.08))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(backgroundColor.opacity(0.25), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .accessibilityIdentifier("provider-troubleshooting-\(report.providerID.uuidString)")
    }

    private var statusPill: some View {
        Text(report.status.displayName)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(backgroundColor.opacity(0.18))
            .clipShape(Capsule())
    }

    private var backgroundColor: Color {
        switch report.status {
        case .healthy:
            return .green
        case .warning:
            return .orange
        case .blocked:
            return .red
        }
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
}
