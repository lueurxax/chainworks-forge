import Testing
import AppKit
@testable import Chainworks_Forge

@Suite("ArtifactPathClipboard", .serialized, .tags(.fast))
@MainActor
struct ArtifactPathClipboardTests {
    @Test("Copy path writes the canonical string to the general pasteboard")
    func copyPathWritesToPasteboard() {
        let path = "/tmp/chainworks/artifacts/proposal_review_summary.json"

        NSPasteboard.general.clearContents()
        ArtifactPathClipboard.copy(path: path)

        #expect(NSPasteboard.general.string(forType: .string) == path)
    }
}
