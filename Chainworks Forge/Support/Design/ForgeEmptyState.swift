import SwiftUI

struct ForgeEmptyState: View {
    let title: String
    let systemImage: String
    var description: String?
    var actionTitle: String?
    var action: (() -> Void)?

    var body: some View {
        ContentUnavailableView {
            Label {
                Text(title)
            } icon: {
                ForgeIconBridge.symbol(systemImage)
            }
                .symbolRenderingMode(.multicolor)
                .font(ForgeTypography.emptyStateIcon)
        } description: {
            if let description {
                Text(description)
                    .font(ForgeTypography.body)
            }
        } actions: {
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.regular)
            }
        }
    }
}
