import Foundation
import AppKit

enum ArtifactPathClipboard {
    static func copy(path: String) {
        // SEC-P083-MED-002: use currentHostOnly to prevent Universal Clipboard propagation of paths.
        NSPasteboard.general.prepareForNewContents(with: .currentHostOnly)
        NSPasteboard.general.setString(path, forType: .string)
    }
}
