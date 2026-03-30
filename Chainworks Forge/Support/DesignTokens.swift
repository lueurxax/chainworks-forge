import SwiftUI

// MARK: - Proposal 014 / Backward-Compatible Facade
//
// The canonical shared primitives now live under `Support/Design/Forge*`.
// `DesignTokens` remains as a compatibility facade so existing adopters keep
// compiling while the codebase stays on a single design authority.
enum DesignTokens {
    enum Brand {
        static let forgeBlue = Color(red: 11.0 / 255.0, green: 31.0 / 255.0, blue: 42.0 / 255.0)
        static let forgeBlueSoft = Color(red: 19.0 / 255.0, green: 47.0 / 255.0, blue: 63.0 / 255.0)
        static let accent = Color(red: 1.0, green: 138.0 / 255.0, blue: 0.0)
        static let accentSoft = Color(red: 1.0, green: 179.0 / 255.0, blue: 71.0 / 255.0)
    }

    enum Neutral {
        static let canvas = Color(nsColor: .windowBackgroundColor)
        static let surface = Color(nsColor: .controlBackgroundColor)
        static let panel = Color(nsColor: .textBackgroundColor)
        static let panelSubtle = Color.primary.opacity(0.035)
        static let brandWash = Brand.forgeBlueSoft.opacity(0.08)
        static let accentWash = Brand.accent.opacity(0.10)
        static let outline = Color.primary.opacity(0.10)
        static let quietOutline = Color.primary.opacity(0.06)
        static let textSecondary = Color.secondary
        static let textTertiary = Color.secondary.opacity(0.8)
    }

    // MARK: - Semantic Status Colors (M-02)
    enum Status {
        static let success = ForgeStatusColor.success
        static let warning = ForgeStatusColor.warning
        static let error = ForgeStatusColor.error
        static let running = ForgeStatusColor.running
        static let neutral = ForgeStatusColor.neutral
        static let cancelled = ForgeStatusColor.cancelled
    }

    enum Action {
        static let primary = ForgeColor.Brand.accent
        static let destructive = ForgeStatusColor.error
        static let approve = ForgeStatusColor.success
        static let caution = ForgeStatusColor.warning
    }

    enum Spacing {
        static let compact = ForgeSpacing.compact
        static let small = ForgeSpacing.small
        static let medium = ForgeSpacing.medium
        static let large = ForgeSpacing.large
        static let section = ForgeSpacing.section
    }

    enum CornerRadius {
        static let card = ForgeRadius.card
        static let panel = ForgeRadius.panel
    }

    enum Shadow {
        static let cardColor = Color.black.opacity(0.08)
        static let cardRadius: CGFloat = 8
        static let cardYOffset: CGFloat = 2
    }

    // MARK: - Badge Opacity

    /// Unified background opacity for status capsules and badges.
    static let badgeBackgroundOpacity: Double = 0.15

    // MARK: - Typography Scale (M-03)
    //
    // | Semantic        | SwiftUI font                         | Usage                          |
    // |-----------------|--------------------------------------|--------------------------------|
    // | Screen title    | .title2.bold()                       | NavigationTitle (system)       |
    // | Section header  | .headline                            | GroupBox/Section titles         |
    // | Card title      | .subheadline.weight(.semibold)       | Row titles, provider names     |
    // | Body            | .body                                | Primary content text           |
    // | Supporting      | .caption                             | Secondary/descriptive text     |
    // | Micro           | .caption2                            | Timestamps, metadata, badges   |

    enum Typography {
        static let screenTitle = ForgeTypography.screenTitle
        static let sectionHeader = ForgeTypography.sectionHeader
        static let cardTitle = ForgeTypography.cardTitle
        static let body = ForgeTypography.body
        static let supporting = ForgeTypography.supporting
        static let micro = ForgeTypography.micro
    }

    enum Motion {
        static let quickDuration = 0.16
        static let standardDuration = 0.24
        static let emphasisDuration = 0.32

        static let quick = Animation.easeInOut(duration: quickDuration)
        static let standard = Animation.easeInOut(duration: standardDuration)
        static let emphasis = Animation.easeInOut(duration: emphasisDuration)
    }
}
