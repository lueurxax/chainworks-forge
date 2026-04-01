import SwiftUI

struct StrategyBadge: View {
    let profileID: String?
    let assignmentMode: String?
    let recommendationState: String?

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "chart.bar.xaxis")
                .font(.caption2)
            Text(profileIDLabel)
                .font(.caption.weight(.semibold))
            if let assignmentMode, !assignmentMode.isEmpty {
                Text(assignmentMode.replacingOccurrences(of: "_", with: " "))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            if let recommendationState, !recommendationState.isEmpty {
                Text(recommendationState.replacingOccurrences(of: "_", with: " "))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(
            Capsule()
                .fill(Color.accentColor.opacity(0.12))
        )
        .overlay(
            Capsule()
                .stroke(Color.accentColor.opacity(0.25), lineWidth: 1)
        )
        .accessibilityIdentifier("strategy-badge")
    }

    private var profileIDLabel: String {
        guard let profileID, !profileID.isEmpty else {
            return "No strategy"
        }
        return profileID
    }
}
