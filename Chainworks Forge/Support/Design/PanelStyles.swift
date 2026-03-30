import SwiftUI

enum ForgePanelTone {
    case standard
    case quiet
    case brand
    case success
    case warning
    case critical

    fileprivate var fill: Color {
        switch self {
        case .standard:
            return DesignTokens.Neutral.surface
        case .quiet:
            return DesignTokens.Neutral.panelSubtle
        case .brand:
            return DesignTokens.Neutral.brandWash
        case .success:
            return DesignTokens.Status.success.opacity(0.10)
        case .warning:
            return DesignTokens.Neutral.accentWash
        case .critical:
            return DesignTokens.Status.error.opacity(0.08)
        }
    }

    fileprivate var stroke: Color {
        switch self {
        case .standard:
            return DesignTokens.Neutral.quietOutline
        case .quiet:
            return DesignTokens.Neutral.quietOutline
        case .brand:
            return DesignTokens.Brand.forgeBlueSoft.opacity(0.18)
        case .success:
            return DesignTokens.Status.success.opacity(0.22)
        case .warning:
            return DesignTokens.Brand.accent.opacity(0.22)
        case .critical:
            return DesignTokens.Status.error.opacity(0.22)
        }
    }
}

private struct ForgePanelModifier: ViewModifier {
    let tone: ForgePanelTone
    let padding: CGFloat
    let cornerRadius: CGFloat

    func body(content: Content) -> some View {
        content
            .padding(padding)
            .background(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .fill(tone.fill)
            )
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(tone.stroke, lineWidth: 1)
            )
            .shadow(
                color: DesignTokens.Shadow.cardColor,
                radius: DesignTokens.Shadow.cardRadius,
                y: DesignTokens.Shadow.cardYOffset
            )
    }
}

private struct ForgeSelectionCardModifier: ViewModifier {
    let isSelected: Bool

    func body(content: Content) -> some View {
        content
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(isSelected ? DesignTokens.Neutral.brandWash : DesignTokens.Neutral.panelSubtle)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(
                        isSelected ? DesignTokens.Brand.forgeBlueSoft.opacity(0.28) : DesignTokens.Neutral.quietOutline,
                        lineWidth: 1
                    )
            )
    }
}

extension View {
    func forgePanel(
        tone: ForgePanelTone = .standard,
        padding: CGFloat = DesignTokens.Spacing.medium,
        cornerRadius: CGFloat = DesignTokens.CornerRadius.panel
    ) -> some View {
        modifier(ForgePanelModifier(tone: tone, padding: padding, cornerRadius: cornerRadius))
    }

    func forgeInsetPanel(tone: ForgePanelTone = .quiet) -> some View {
        modifier(
            ForgePanelModifier(
                tone: tone,
                padding: DesignTokens.Spacing.small,
                cornerRadius: DesignTokens.CornerRadius.card
            )
        )
    }

    func forgeSelectionCard(isSelected: Bool) -> some View {
        modifier(ForgeSelectionCardModifier(isSelected: isSelected))
    }
}
