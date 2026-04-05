import Foundation
import AppKit

enum ArtifactPathClipboard {
    static func copy(path: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(path, forType: .string)
    }
}
