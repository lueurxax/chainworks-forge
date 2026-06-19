import SwiftUI

struct ForgePanelStyle: ViewModifier {
    var tint: Color = ForgeColor.Surface.border
    var fill: Color = ForgeColor.Surface.elevated

    func body(content: Content) -> some View {
        content
            .padding(ForgeSpacing.large)
            .forgeGlassSurface(.panel, tint: tint, fill: fill)
    }
}

extension View {
    func forgePanel(tint: Color = ForgeColor.Surface.border, fill: Color = ForgeColor.Surface.elevated) -> some View {
        modifier(ForgePanelStyle(tint: tint, fill: fill))
    }
}
