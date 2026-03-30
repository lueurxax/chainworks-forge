import SwiftUI

struct ForgeSectionHeader: View {
    let title: String
    var subtitle: String?
    var symbol: String?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: ForgeSpacing.small) {
            if let symbol {
                Image(systemName: symbol)
                    .foregroundStyle(ForgeColor.Brand.accent)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(ForgeTypography.sectionHeader)
                if let subtitle {
                    Text(subtitle)
                        .font(ForgeTypography.supporting)
                        .foregroundStyle(ForgeColor.Text.secondary)
                }
            }
            Spacer()
        }
    }
}
