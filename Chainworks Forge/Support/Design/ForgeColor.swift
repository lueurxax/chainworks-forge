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

    enum Status {
        static let success = Color(red: 0.20, green: 0.78, blue: 0.35)
        static let warning = Color(red: 1.00, green: 0.62, blue: 0.04)
        static let approval = Color(red: 1.00, green: 0.84, blue: 0.04)
        static let error = Color(red: 1.00, green: 0.23, blue: 0.19)
        static let running = Color(red: 0.04, green: 0.52, blue: 1.00)
        static let neutral = Color.secondary
        static let cancelled = Color.gray
    }
}
