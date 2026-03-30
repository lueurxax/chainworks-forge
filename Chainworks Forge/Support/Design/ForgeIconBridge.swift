import SwiftUI

enum ForgeIconBridge {
    enum BrandAsset: String, CaseIterable, Sendable {
        case mark = "BrandMark"
        case horizontalLogo = "BrandHorizontalLogo"
        case hero = "BrandHero"
    }

    static func symbol(_ name: String) -> Image {
        Image(systemName: name)
    }

    static func statusSymbol(for status: RunStatus) -> String {
        switch status {
        case .pending, .ready:
            return "clock"
        case .running:
            return "bolt.circle.fill"
        case .waitingApproval:
            return "checkmark.seal.fill"
        case .blocked:
            return "pause.circle.fill"
        case .completed:
            return "checkmark.circle.fill"
        case .failed:
            return "xmark.circle.fill"
        case .cancelling:
            return "hourglass"
        case .cancelled:
            return "slash.circle.fill"
        }
    }

    static func brand(_ asset: BrandAsset) -> Image {
        Image(asset.rawValue)
    }

    static func brandMark() -> Image {
        brand(.mark)
    }

    static func brandHorizontalLogo() -> Image {
        brand(.horizontalLogo)
    }

    static func brandHero() -> Image {
        brand(.hero)
    }

    static func artifactSymbol(for format: ArtifactFormat) -> String {
        switch format {
        case .json:
            return "curlybraces"
        case .markdown:
            return "doc.text"
        case .diff:
            return "arrow.left.arrow.right"
        case .report:
            return "doc.text.fill"
        }
    }
}
