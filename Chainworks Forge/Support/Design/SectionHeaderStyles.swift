import SwiftUI

struct ForgeSectionHeader: View {
    let title: String
    var subtitle: String? = nil
    var systemImage: String? = nil
    var tint: Color = DesignTokens.Brand.forgeBlueSoft

    var body: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
            HStack(spacing: DesignTokens.Spacing.small) {
                if let systemImage {
                    Image(systemName: systemImage)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(tint)
                }
                Text(title)
                    .font(DesignTokens.Typography.sectionHeader.weight(.semibold))
                    .foregroundStyle(.primary)
            }

            if let subtitle, !subtitle.isEmpty {
                Text(subtitle)
                    .font(DesignTokens.Typography.supporting)
                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct ForgeIdentityHeader: View {
    let title: String
    var subtitle: String? = nil
    var surfaceRole: BrandSurfaceRole = .setupIdentity
    var style: BrandMarkStyle = .symbol

    var body: some View {
        HStack(alignment: .top, spacing: DesignTokens.Spacing.medium) {
            BrandMarkView(style: style, surfaceRole: surfaceRole, maxHeight: style.defaultMaxHeight + 6)

            VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
                Text(title)
                    .font(.title2.weight(.bold))
                    .foregroundStyle(.primary)

                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier(accessibilityIdentifier)
    }

    private var accessibilityIdentifier: String {
        "forge-identity-header-" + title.lowercased().replacingOccurrences(of: " ", with: "-")
    }
}
