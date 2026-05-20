import SwiftUI

struct ForgeSkeleton: View {
    let width: CGFloat?
    let height: CGFloat
    let cornerRadius: CGFloat

    init(width: CGFloat? = nil, height: CGFloat = 16, cornerRadius: CGFloat = 4) {
        self.width = width
        self.height = height
        self.cornerRadius = cornerRadius
    }

    var body: some View {
        RoundedRectangle(cornerRadius: cornerRadius)
            .fill(Color.secondary.opacity(0.12))
            .frame(width: width, height: height)
    }
}

extension ForgeSkeleton {
    static func text(width: CGFloat? = nil) -> some View {
        ForgeSkeleton(width: width, height: 14)
    }

    static func headline(width: CGFloat? = nil) -> some View {
        ForgeSkeleton(width: width, height: 18, cornerRadius: 6)
    }

    static func card() -> some View {
        ForgeSkeleton(width: .infinity, height: 120, cornerRadius: 12)
    }
}
