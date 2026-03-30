import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 014 Design System", .serialized, .tags(.fast))
struct Proposal014Tests {
    @Test func canonicalBrandAssetNamesRemainStable() {
        #expect(BrandAssetName.logoHorizontal.rawValue == "chainworks-forge-logo-horizontal")
        #expect(BrandAssetName.appIcon.rawValue == "chainworks-forge-app-icon")
        #expect(BrandAssetName.symbolMonochrome.rawValue == "chainworks-forge-symbol-monochrome")
        #expect(BrandAssetName.heroDark.rawValue == "chainworks-forge-hero-dark")
        #expect(BrandAssetName.heroLight.rawValue == "chainworks-forge-hero-light")
    }

    @Test func iconUsageRulesPreferSystemSymbolsForDenseOperationalControls() {
        #expect(IconUsageRules.assetUsage(for: .denseOperationalControl) == .sfSymbolOnly)
        #expect(IconUsageRules.assetUsage(for: .runtimePanel) == .sfSymbolOnly)
        #expect(IconUsageRules.assetUsage(for: .toolbarBranding) == .symbolOnly)
        #expect(IconUsageRules.assetUsage(for: .setupIdentity) == .symbolOnly)
        #expect(IconUsageRules.assetUsage(for: .documentationHero) == .fullLogo)
    }

    @Test func orangeAccentRemainsBounded() {
        #expect(IconUsageRules.allowsOrangeAccent(in: .documentationHero))
        #expect(IconUsageRules.allowsOrangeAccent(in: .setupIdentity))
        #expect(!IconUsageRules.allowsOrangeAccent(in: .runtimePanel))
        #expect(!IconUsageRules.allowsOrangeAccent(in: .denseOperationalControl))
    }

    @Test func brandMarkAccessibilityIdentifiersAreDeterministic() {
        #expect(BrandMarkStyle.fullLogo.accessibilityIdentifier == "brand-mark-horizontal-logo")
        #expect(BrandMarkStyle.symbol.accessibilityIdentifier == "brand-mark-symbol")
    }
}
