import SwiftUI

enum BrandAssetName: String, CaseIterable {
    case logoHorizontal = "chainworks-forge-logo-horizontal"
    case appIcon = "chainworks-forge-app-icon"
    case symbolMonochrome = "chainworks-forge-symbol-monochrome"
    case heroDark = "chainworks-forge-hero-dark"
    case heroLight = "chainworks-forge-hero-light"
}

enum BrandSurfaceRole {
    case documentationHero
    case shellIdentity
    case setupIdentity
    case toolbarBranding
    case denseOperationalControl
    case runtimePanel
}

enum BrandAssetUsage: Equatable {
    case fullLogo
    case symbolOnly
    case sfSymbolOnly
    case none
}

enum BrandMarkStyle: Equatable {
    case fullLogo
    case symbol

    var assetName: BrandAssetName {
        switch self {
        case .fullLogo:
            return .logoHorizontal
        case .symbol:
            return .symbolMonochrome
        }
    }

    var accessibilityIdentifier: String {
        switch self {
        case .fullLogo:
            return "brand-mark-horizontal-logo"
        case .symbol:
            return "brand-mark-symbol"
        }
    }

    var defaultMaxHeight: CGFloat {
        switch self {
        case .fullLogo:
            return 34
        case .symbol:
            return 24
        }
    }
}

enum IconUsageRules {
    static func assetUsage(for role: BrandSurfaceRole) -> BrandAssetUsage {
        switch role {
        case .documentationHero:
            return .fullLogo
        case .shellIdentity, .setupIdentity, .toolbarBranding:
            return .symbolOnly
        case .denseOperationalControl, .runtimePanel:
            return .sfSymbolOnly
        }
    }

    static func allowsOrangeAccent(in role: BrandSurfaceRole) -> Bool {
        switch role {
        case .documentationHero, .setupIdentity, .shellIdentity:
            return true
        case .toolbarBranding, .denseOperationalControl, .runtimePanel:
            return false
        }
    }
}

struct BrandMarkView: View {
    let style: BrandMarkStyle
    var surfaceRole: BrandSurfaceRole
    var maxHeight: CGFloat? = nil

    var body: some View {
        let usage = IconUsageRules.assetUsage(for: surfaceRole)

        Group {
            switch usage {
            case .fullLogo where style == .fullLogo:
                brandImage(for: .fullLogo)
            case .symbolOnly:
                brandImage(for: .symbol)
            case .fullLogo:
                brandImage(for: .fullLogo)
            case .sfSymbolOnly, .none:
                EmptyView()
            }
        }
    }

    @ViewBuilder
    private func brandImage(for markStyle: BrandMarkStyle) -> some View {
        Image(markStyle.assetName.rawValue)
            .resizable()
            .interpolation(.high)
            .antialiased(true)
            .scaledToFit()
            .frame(maxHeight: maxHeight ?? markStyle.defaultMaxHeight)
            .accessibilityHidden(true)
            .accessibilityIdentifier(markStyle.accessibilityIdentifier)
    }
}
