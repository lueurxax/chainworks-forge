import SwiftUI

// MARK: - Proposal 014 / Backward-Compatible Facade
//
// The canonical shared primitives now live under `Support/Design/Forge*`.
// `DesignTokens` remains as a compatibility facade so existing adopters keep
// compiling while the codebase stays on a single design authority.
enum DesignTokens {
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

    static let badgeBackgroundOpacity = ForgeColor.Badge.backgroundOpacity

    enum Typography {
        static let screenTitle = ForgeTypography.screenTitle
        static let sectionHeader = ForgeTypography.sectionHeader
        static let cardTitle = ForgeTypography.cardTitle
        static let body = ForgeTypography.body
        static let supporting = ForgeTypography.supporting
        static let micro = ForgeTypography.micro
    }
}
