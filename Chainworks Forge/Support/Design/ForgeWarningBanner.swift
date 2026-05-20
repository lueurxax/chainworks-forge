import SwiftUI

struct ForgeWarningBanner: View {
    let message: String
    let systemImage: String
    let tint: Color

    init(_ message: String, systemImage: String = "exclamationmark.triangle.fill", tint: Color = .orange) {
        self.message = message
        self.systemImage = systemImage
        self.tint = tint
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: systemImage)
                .foregroundStyle(tint)
                .font(.subheadline.weight(.semibold))
            
            Text(message)
                .font(.subheadline)
                .foregroundStyle(.primary)
                .textSelection(.enabled)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tint.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(tint.opacity(0.2), lineWidth: 1)
        )
    }
}

extension ForgeWarningBanner {
    static func error(_ message: String) -> some View {
        ForgeWarningBanner(message, systemImage: "xmark.octagon.fill", tint: .red)
    }
    
    static func info(_ message: String) -> some View {
        ForgeWarningBanner(message, systemImage: "info.circle.fill", tint: .blue)
    }
}
