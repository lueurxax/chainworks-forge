import SwiftUI

struct ForgePanelStyle: ViewModifier {
    var tint: Color = ForgeColor.Surface.border
    var fill: Color = ForgeColor.Surface.elevated

    func body(content: Content) -> some View {
        content
            .padding(ForgeSpacing.large)
            .background(fill, in: RoundedRectangle(cornerRadius: ForgeRadius.panel, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: ForgeRadius.panel, style: .continuous)
                    .strokeBorder(tint.opacity(0.4), lineWidth: 1)
            )
    }
}

extension View {
    func forgePanel(tint: Color = ForgeColor.Surface.border, fill: Color = ForgeColor.Surface.elevated) -> some View {
        modifier(ForgePanelStyle(tint: tint, fill: fill))
    }
}
