import Testing
import AppKit
@testable import Chainworks_Forge

@Suite("ArtifactContentRenderer", .tags(.fast))
struct ArtifactContentRendererTests {
    @Test("Markdown text view skips redundant attributed string updates")
    func markdownTextViewSkipsRedundantAttributedStringUpdates() {
        let text = NSAttributedString(string: "Hello")

        #expect(MarkdownTextViewUpdatePolicy.needsAttributedStringUpdate(current: nil, incoming: text))
        #expect(MarkdownTextViewUpdatePolicy.needsAttributedStringUpdate(current: text, incoming: text) == false)
        #expect(
            MarkdownTextViewUpdatePolicy.needsAttributedStringUpdate(
                current: NSAttributedString(string: "Hello"),
                incoming: NSAttributedString(string: "World")
            )
        )
    }

    @Test("Markdown text view invalidates layout only for meaningful width changes")
    func markdownTextViewInvalidatesOnlyForMeaningfulWidthChanges() {
        #expect(MarkdownTextViewUpdatePolicy.shouldInvalidateLayout(previousWidth: nil, newWidth: 240))
        #expect(MarkdownTextViewUpdatePolicy.shouldInvalidateLayout(previousWidth: 240, newWidth: 240.2) == false)
        #expect(MarkdownTextViewUpdatePolicy.shouldInvalidateLayout(previousWidth: 240, newWidth: 244))
    }
}
