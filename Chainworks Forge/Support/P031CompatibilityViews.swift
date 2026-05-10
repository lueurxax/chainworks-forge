import SwiftUI

struct P031OperatorPlaceholder: View {
    let title: String
    let message: String
    let identifier: String
    let titleIdentifier: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.title2.weight(.semibold))
                .accessibilityIdentifier(titleIdentifier)
            Text(message)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(24)
        .accessibilityIdentifier(identifier)
    }
}

struct P031AccessibilityMarker: View {
    let identifier: String

    var body: some View {
        Text(" ")
            .font(.system(size: 1))
            .frame(width: 1, height: 1)
            .foregroundStyle(.clear)
            .accessibilityLabel(identifier)
            .accessibilityIdentifier(identifier)
    }
}
