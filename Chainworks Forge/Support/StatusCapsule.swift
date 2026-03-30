import SwiftUI

// MARK: - Proposal 012 §4 / M-01: Unified Status Capsule

/// Reusable badge component that replaces fragmented badge
/// implementations across the adopter slice.
///
/// Accessibility: preserves textual label so status is never
/// conveyed by color alone. Legible under Increase Contrast
/// and Reduce Transparency via `.foregroundStyle` + opaque text.
struct StatusCapsule: View {
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings

    let text: String
    let color: Color
    var icon: String?
    var size: Size = .regular
    var accessibilityIdentifier: String?

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
        .background(backgroundFill, in: Capsule())
        .overlay {
            Capsule()
                .strokeBorder(borderColor, lineWidth: borderLineWidth)
        }
        .overlay(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 1) {
                ForEach(activeAccessibilitySettingIdentifiers, id: \.self) { identifier in
                    Color.clear
                        .frame(width: 1, height: 1)
                        .accessibilityIdentifier(identifier)
                }
            }
        }
        .foregroundStyle(color)
        // Proposal 012 (REQ-005): Explicit VoiceOver semantics —
        // combine icon + text into a single spoken element so status
        // is announced as text, not conveyed by color alone.
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isStaticText)
        .accessibilityLabel(text)
        .accessibilityValue(accessibilitySettingsDescription)
        .accessibilityIdentifier(accessibilityIdentifier ?? text)
    }

    private var fontSize: Font {
        switch size {
        case .small:
            return ForgeTypography.statusCapsuleSmall
        case .regular:
            return ForgeTypography.statusCapsule
        }
    }

    private var isIncreasedContrast: Bool {
        uiTestAccessibilitySettings.increaseContrast
    }

    private var backgroundFill: Color {
        if reduceTransparency {
            return color.opacity(isIncreasedContrast ? 0.34 : 0.28)
        }
        if isIncreasedContrast {
            return color.opacity(0.24)
        }
        return color.opacity(ForgeColor.Badge.backgroundOpacity)
    }

    private var borderLineWidth: CGFloat {
        if differentiateWithoutColor && isIncreasedContrast {
            return 2
        }
        if differentiateWithoutColor || isIncreasedContrast || reduceTransparency {
            return 1
        }
        return 0
    }

    private var borderColor: Color {
        differentiateWithoutColor || isIncreasedContrast || reduceTransparency ? color : .clear
    }

    private var accessibilitySettingsDescription: String {
        var activeModes: [String] = []
        if differentiateWithoutColor {
            activeModes.append("differentiate without color")
        }
        if isIncreasedContrast {
            activeModes.append("increase contrast")
        }
        if reduceTransparency {
            activeModes.append("reduce transparency")
        }
        return activeModes.isEmpty ? "standard accessibility display settings" : activeModes.joined(separator: ", ")
    }

    private var activeAccessibilitySettingIdentifiers: [String] {
        var identifiers: [String] = []
        if differentiateWithoutColor {
            identifiers.append("\(sanitizedIdentifier)-differentiate-without-color")
        }
        if isIncreasedContrast {
            identifiers.append("\(sanitizedIdentifier)-increase-contrast")
        }
        if reduceTransparency {
            identifiers.append("\(sanitizedIdentifier)-reduce-transparency")
        }
        return identifiers
    }

    private var sanitizedIdentifier: String {
        (accessibilityIdentifier ?? text)
            .lowercased()
            .replacingOccurrences(of: " ", with: "-")
            .replacingOccurrences(of: "/", with: "-")
    }

    private var differentiateWithoutColor: Bool {
        uiTestAccessibilitySettings.differentiateWithoutColor
    }

    private var reduceTransparency: Bool {
        uiTestAccessibilitySettings.reduceTransparency
    }
}

// MARK: - Preview

#Preview("StatusCapsule Sizes") {
    VStack(spacing: 12) {
        HStack(spacing: 8) {
            StatusCapsule(text: "Running", color: ForgeStatusColor.running, icon: "play.circle.fill", size: .small)
            StatusCapsule(text: "Completed", color: ForgeStatusColor.success, icon: "checkmark.circle.fill", size: .small)
            StatusCapsule(text: "Failed", color: ForgeStatusColor.error, icon: "xmark.circle.fill", size: .small)
        }
        HStack(spacing: 8) {
            StatusCapsule(text: "Awaiting Approval", color: ForgeStatusColor.warning, icon: "checkmark.seal")
            StatusCapsule(text: "Blocked", color: ForgeStatusColor.error, icon: "exclamationmark.triangle.fill")
            StatusCapsule(text: "Cancelled", color: ForgeStatusColor.cancelled)
        }
    }
    .padding()
}
