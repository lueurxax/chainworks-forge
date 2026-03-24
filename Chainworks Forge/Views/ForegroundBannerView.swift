import SwiftUI

// MARK: - P005-OPS §10: Foreground Banner

/// Active-app foreground banner surface.
/// Shows a non-intrusive but visible banner when the app is in the foreground
/// and there are runs requiring operator attention (approval, blocked, failed).
/// Tapping the banner navigates to the Runs Home tab.
struct ForegroundBannerView: View {
    let waitingApprovalCount: Int
    let blockedCount: Int
    let failedCount: Int
    let onTap: () -> Void

    private var totalAttention: Int {
        waitingApprovalCount + blockedCount + failedCount
    }

    private var bannerText: String {
        var parts: [String] = []
        if waitingApprovalCount > 0 {
            parts.append("\(waitingApprovalCount) awaiting approval")
        }
        if blockedCount > 0 {
            parts.append("\(blockedCount) blocked")
        }
        if failedCount > 0 {
            parts.append("\(failedCount) failed")
        }
        return parts.joined(separator: " · ")
    }

    private var bannerIcon: String {
        if waitingApprovalCount > 0 { return "checkmark.seal.fill" }
        if blockedCount > 0 { return "exclamationmark.triangle.fill" }
        return "xmark.circle.fill"
    }

    private var bannerColor: Color {
        if blockedCount > 0 || failedCount > 0 { return .red }
        return .orange
    }

    var body: some View {
        if totalAttention > 0 {
            Button(action: onTap) {
                HStack(spacing: 8) {
                    Image(systemName: bannerIcon)
                        .foregroundStyle(.white)
                    Text(bannerText)
                        .font(.caption.bold())
                        .foregroundStyle(.white)
                    Spacer()
                    Text("View in Runs Home →")
                        .font(.caption2)
                        .foregroundStyle(.white.opacity(0.8))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(bannerColor.gradient)
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 12)
            .padding(.top, 4)
            .transition(.move(edge: .top).combined(with: .opacity))
            .accessibilityIdentifier("foreground-attention-banner")
        }
    }
}
