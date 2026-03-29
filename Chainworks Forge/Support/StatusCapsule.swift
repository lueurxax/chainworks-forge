import SwiftUI

// MARK: - Proposal 012 §4 / M-01: Unified Status Capsule

/// Reusable badge component that replaces fragmented badge
/// implementations across the adopter slice.
///
/// Accessibility: preserves textual label so status is never
/// conveyed by color alone. Legible under Increase Contrast
/// and Reduce Transparency via `.foregroundStyle` + opaque text.
struct StatusCapsule: View {
    let text: String
    let color: Color
    var icon: String?
    var size: Size = .regular

    enum Size {
        /// caption2, px:6 / py:2 — compact inline badges
        case small
        /// caption2.bold, px:8 / py:3 — standard badges
        case regular
    }

    var body: some View {
        HStack(spacing: size == .small ? 3 : 4) {
            if let icon {
                Image(systemName: icon)
                    .font(fontSize)
            }
            Text(text)
                .font(fontSize)
        }
        .padding(.horizontal, size == .small ? 6 : 8)
        .padding(.vertical, size == .small ? 2 : 3)
        .background(color.opacity(DesignTokens.badgeBackgroundOpacity), in: Capsule())
        .foregroundStyle(color)
        // Proposal 012 (REQ-005): Explicit VoiceOver semantics —
        // combine icon + text into a single spoken element so status
        // is announced as text, not conveyed by color alone.
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isStaticText)
        .accessibilityLabel(text)
    }

    private var fontSize: Font {
        switch size {
        case .small:  return .caption2
        case .regular: return .caption2.bold()
        }
    }
}

// MARK: - Preview

#Preview("StatusCapsule Sizes") {
    VStack(spacing: 12) {
        HStack(spacing: 8) {
            StatusCapsule(text: "Running", color: DesignTokens.Status.running, icon: "play.circle.fill", size: .small)
            StatusCapsule(text: "Completed", color: DesignTokens.Status.success, icon: "checkmark.circle.fill", size: .small)
            StatusCapsule(text: "Failed", color: DesignTokens.Status.error, icon: "xmark.circle.fill", size: .small)
        }
        HStack(spacing: 8) {
            StatusCapsule(text: "Awaiting Approval", color: DesignTokens.Status.warning, icon: "checkmark.seal")
            StatusCapsule(text: "Blocked", color: DesignTokens.Status.error, icon: "exclamationmark.triangle.fill")
            StatusCapsule(text: "Cancelled", color: DesignTokens.Status.cancelled)
        }
    }
    .padding()
}
