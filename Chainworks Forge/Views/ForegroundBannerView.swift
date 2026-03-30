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
        if blockedCount > 0 || failedCount > 0 { return DesignTokens.Status.error }
        return DesignTokens.Action.caution
    }

    var body: some View {
        if totalAttention > 0 {
            Button(action: onTap) {
                HStack(spacing: 8) {
                    Image(systemName: bannerIcon)
                        .foregroundStyle(.white)
                    Text(bannerText)
                        .font(DesignTokens.Typography.supporting.weight(.bold))
                        .foregroundStyle(.white)
                    Spacer()
                    Text("View in Runs Home →")
                        .font(DesignTokens.Typography.micro)
                        .foregroundStyle(.white.opacity(0.8))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(
                    RoundedRectangle(cornerRadius: DesignTokens.CornerRadius.card, style: .continuous)
                        .fill(LinearGradient(colors: [bannerColor, bannerColor.opacity(0.82)], startPoint: .leading, endPoint: .trailing))
                )
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 12)
            .padding(.top, 4)
            // Proposal 012 (M-04): Banner is positioned at .bottom via overlay alignment,
            // so the transition must slide from the bottom edge, not the top.
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .accessibilityIdentifier("foreground-attention-banner")
        }
    }
}
