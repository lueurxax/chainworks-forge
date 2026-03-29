import SwiftUI

// MARK: - Proposal 012 §4: Design System Foundation

/// Centralised semantic design tokens for Chainworks Forge.
/// Adopted first by the Phase 3 bounded adopter slice
/// (`RunsHomeView`, `WorkflowMapView`, `ReleaseGateView`,
/// `DeliveryPreflightReportView`, `IdeaListView`).
/// Expansion beyond the adopter slice requires verification
/// per §4.4 guardrails.
enum DesignTokens {

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
        static let primary     = Color.accentColor
        static let destructive = Color.red
        static let approve     = Color.green
        static let caution     = Color.orange
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
}
