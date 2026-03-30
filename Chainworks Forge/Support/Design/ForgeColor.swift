import SwiftUI
#if os(macOS)
import AppKit
#endif

enum ForgeColor {
    enum Surface {
        static let appBackground = Color(nsColor: .windowBackgroundColor)
        static let elevated = Color(nsColor: .controlBackgroundColor)
        static let muted = Color.secondary.opacity(0.08)
        static let border = Color.secondary.opacity(0.18)
    }

    enum Text {
        static let primary = Color.primary
        static let secondary = Color.secondary
        static let tertiary = Color.secondary.opacity(0.78)
    }

    enum Brand {
        static let accent = Color.accentColor
        static let accentMuted = Color.accentColor.opacity(0.08)
    }

    enum Badge {
        static let backgroundOpacity: Double = 0.15
    }
}
