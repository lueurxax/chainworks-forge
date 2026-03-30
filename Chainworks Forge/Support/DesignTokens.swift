import SwiftUI

// MARK: - Proposal 012 §4: Design System Foundation

/// Centralised semantic design tokens for Chainworks Forge.
/// Adopted first by the Phase 3 bounded adopter slice
/// (`RunsHomeView`, `WorkflowMapView`, `ReleaseGateView`,
/// `DeliveryPreflightReportView`, `IdeaListView`).
/// Expansion beyond the adopter slice requires verification
/// per §4.4 guardrails.
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
        static let success   = Color.green
        static let warning   = Color.orange
        static let error     = Color.red
        static let running   = Color.blue
        static let neutral   = Color.secondary
        static let cancelled = Color.gray
    }

    enum Action {
        static let primary     = Brand.forgeBlueSoft
        static let destructive = Color.red
        static let approve     = Color.green
        static let caution     = Brand.accent
    }

    // MARK: - Spacing (§4.2)

    enum Spacing {
        /// 4pt — Tight inline spacing
        static let compact: CGFloat  = 4
        /// 8pt — Between related items
        static let small: CGFloat    = 8
        /// 12pt — Between sections within a group
        static let medium: CGFloat   = 12
        /// 16pt — Between GroupBoxes/sections
        static let large: CGFloat    = 16
        /// 20pt — Between major content blocks
        static let section: CGFloat  = 20
    }

    // MARK: - Corner Radius (§4.3)

    enum CornerRadius {
        /// 14pt continuous — Stage cards, agent panels
        static let card: CGFloat  = 14
        /// 16pt continuous — Larger containers
        static let panel: CGFloat = 16
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
        static let sectionHeader: Font = .headline
        static let cardTitle: Font     = .subheadline.weight(.semibold)
        static let body: Font          = .body
        static let supporting: Font    = .caption
        static let micro: Font         = .caption2
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
