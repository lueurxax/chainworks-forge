import SwiftUI

enum ForgeGlassRole: String, CaseIterable, Sendable {
    case panel
    case chrome
    case sidebar
    case toolbar
    case prominentAction

    var identifier: String {
        switch self {
        case .panel:
            "panel"
        case .chrome:
            "chrome"
        case .sidebar:
            "sidebar"
        case .toolbar:
            "toolbar"
        case .prominentAction:
            "prominent-action"
        }
    }

    var cornerRadius: CGFloat {
        switch self {
        case .panel, .chrome, .sidebar:
            ForgeRadius.panel
        case .toolbar, .prominentAction:
            ForgeRadius.card
        }
    }

    var fallbackFill: Color {
        switch self {
        case .panel, .chrome, .sidebar:
            ForgeColor.Surface.elevated
        case .toolbar, .prominentAction:
            ForgeColor.Surface.muted
        }
    }

    var fallbackTint: Color {
        switch self {
        case .prominentAction:
            ForgeColor.Brand.accent.opacity(0.35)
        default:
            ForgeColor.Surface.border
        }
    }
}

struct ForgeGlassSurfaceStyle: ViewModifier {
    let role: ForgeGlassRole
    var tint: Color?
    var fill: Color?

    func body(content: Content) -> some View {
        content
            .background(surfaceFill, in: shape)
            .overlay {
                shape.strokeBorder(surfaceTint.opacity(0.45), lineWidth: 1)
            }
            .modifier(PlatformLiquidGlassEffect(role: role))
    }

    private var surfaceFill: Color {
        fill ?? role.fallbackFill
    }

    private var surfaceTint: Color {
        tint ?? role.fallbackTint
    }

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: role.cornerRadius, style: .continuous)
    }
}

private struct PlatformLiquidGlassEffect: ViewModifier {
    let role: ForgeGlassRole

    func body(content: Content) -> some View {
        if #available(macOS 26.0, *) {
            content.glassEffect(.regular, in: RoundedRectangle(cornerRadius: role.cornerRadius, style: .continuous))
        } else {
            content
        }
    }
}

extension View {
    func forgeGlassSurface(
        _ role: ForgeGlassRole,
        tint: Color? = nil,
        fill: Color? = nil
    ) -> some View {
        modifier(ForgeGlassSurfaceStyle(role: role, tint: tint, fill: fill))
    }
}
