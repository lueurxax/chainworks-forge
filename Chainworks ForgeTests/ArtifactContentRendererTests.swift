import Testing
import AppKit
@testable import Chainworks_Forge

@Suite("ArtifactContentRenderer", .tags(.fast))
struct ArtifactContentRendererTests {
    @Test("JSON-declared markdown content renders as markdown instead of parse failure")
    func jsonDeclaredMarkdownContentRendersAsMarkdown() {
        let content = """
        # Idea Brief

        ## Goal

        Finish the proposal.
        """

        let intent = ArtifactPresentationIntent.resolve(
            content: content,
            context: .explicitNamed(format: .json, artifactName: "idea_brief.json")
        )

        #expect(intent == .markdownDocument)
    }

    @Test("JSON-declared plain text falls back to raw monospaced text")
    func jsonDeclaredPlainTextRendersAsRawText() {
        let intent = ArtifactPresentationIntent.resolve(
            content: "not a json payload",
            context: .explicitNamed(format: .json, artifactName: "notes.json")
        )

        #expect(intent == .plainText(monospaced: true))
    }

    @Test("JSON-declared structured content still renders as JSON tree")
    func jsonDeclaredStructuredContentRendersAsJSONTree() {
        let intent = ArtifactPresentationIntent.resolve(
            content: #"{"status":"ready"}"#,
            context: .explicitNamed(format: .json, artifactName: "run_state.json")
        )

        #expect(intent == .jsonTree(rescuedFrom: nil))
    }

    @Test("Large markdown artifact is capped before document rendering")
    func largeMarkdownArtifactIsCappedBeforeRendering() {
        let content = (0..<4_000)
            .map { "## Section \($0)\n\nLarge artifact body line \($0)." }
            .joined(separator: "\n\n")

        let prepared = ArtifactPreviewPolicy.prepare(
            content: content,
            intent: .markdownDocument
        )

        #expect(prepared.intent == .markdownDocument)
        #expect(prepared.content.count < content.count)
        #expect(prepared.previewNotice != nil)
        #expect(prepared.content.contains("Section 0"))
    }

    @Test("Large structured JSON artifact uses raw capped preview instead of full tree")
    func largeStructuredJSONUsesRawCappedPreview() {
        let entries = (0..<8_000)
            .map { #""key\#($0)":"value\#($0)""# }
            .joined(separator: ",")
        let content = "{\(entries)}"

        let prepared = ArtifactPreviewPolicy.prepare(
            content: content,
            intent: .jsonTree(rescuedFrom: nil)
        )

        #expect(prepared.intent == .plainText(monospaced: true))
        #expect(prepared.content.count < content.count)
        #expect(prepared.previewNotice != nil)
        #expect(prepared.content.hasPrefix(#"{"key0""#))
    }

    @Test("Large JSON detection avoids building the full JSON tree before preview capping")
    func largeJSONDetectionAvoidsFullTreeBeforePreviewCapping() {
        let entries = (0..<20_000)
            .map { #""key\#($0)":"value\#($0)""# }
            .joined(separator: ",")
        let content = "{\(entries)}"

        let intent = ArtifactPresentationIntent.resolve(
            content: content,
            context: .explicitNamed(format: .json, artifactName: "large.json")
        )
        let prepared = ArtifactPreviewPolicy.prepare(content: content, intent: intent)

        #expect(intent == .jsonTree(rescuedFrom: nil))
        #expect(prepared.intent == .plainText(monospaced: true))
        #expect(prepared.content.count < content.count)
        #expect(prepared.previewNotice?.renderedAsRawText == true)
    }

    @Test("Proposal review summary presentation separates blockers and advisory follow-ups")
    func proposalReviewSummaryPresentationSeparatesBlockersAndAdvisories() {
        let presentation = ProposalReviewSummaryPresentation.parse(
            """
            {
              "pass": true,
              "average_score": 8.8,
              "aggregate_score": 8.8,
              "min_individual_score": 8.2,
              "blocker_count": 0,
              "blocking_issues": [],
              "summary": "approved",
              "blocking_required_changes": [],
              "advisory_follow_ups": ["carry rollout caution into implementation"],
              "recurring_themes": ["durability"],
              "decision": "approved"
            }
            """
        )

        #expect(presentation?.pass == true)
        #expect(presentation?.blockingRequiredChanges.isEmpty == true)
        #expect(presentation?.advisoryFollowUps == ["carry rollout caution into implementation"])
    }

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

  // MARK: - SEC-001 regression: relative path through symlinked ancestor directory

  @Test("Markdown image source policy rejects relative path under symlinked ancestor directory")
  func markdownImageSourcePolicyRejectsRelativePathUnderSymlinkedAncestor() throws {
    let fm = FileManager.default
    let tempRoot = fm.temporaryDirectory
      .appendingPathComponent("p036-test-ancestor-root-\(UUID().uuidString)")
    let outsideDir = fm.temporaryDirectory
      .appendingPathComponent("p036-test-ancestor-outside-\(UUID().uuidString)")
    try fm.createDirectory(at: tempRoot, withIntermediateDirectories: true)
    try fm.createDirectory(at: outsideDir, withIntermediateDirectories: true)
    defer {
      try? fm.removeItem(at: tempRoot)
      try? fm.removeItem(at: outsideDir)
    }
    let secretFile = outsideDir.appendingPathComponent("secret.png")
    try Data([0x89, 0x50, 0x4E, 0x47]).write(to: secretFile)
    // Symlinked directory inside root pointing to the outside directory.
    let symlinkDir = tempRoot.appendingPathComponent("subdir")
    try fm.createSymbolicLink(at: symlinkDir, withDestinationURL: outsideDir)

    let result = MarkdownImageSourcePolicy.v1.resolve(
      source: "subdir/secret.png",
      localRoots: [tempRoot]
    )
    #expect(result == nil, "Relative path through symlinked ancestor directory must be rejected")
  }

  // MARK: - SEC-002 regression: symlink escape outside allowed root

  @Test("Markdown image source policy rejects symlink pointing outside allowed root")
  func markdownImageSourcePolicyRejectsSymlinkEscape() throws {
    let fm = FileManager.default
    let tempRoot = fm.temporaryDirectory
      .appendingPathComponent("p036-test-symlink-root-\(UUID().uuidString)")
    let outsideDir = fm.temporaryDirectory
      .appendingPathComponent("p036-test-outside-\(UUID().uuidString)")
    try fm.createDirectory(at: tempRoot, withIntermediateDirectories: true)
    try fm.createDirectory(at: outsideDir, withIntermediateDirectories: true)
    defer {
      try? fm.removeItem(at: tempRoot)
      try? fm.removeItem(at: outsideDir)
    }
    let outsideImage = outsideDir.appendingPathComponent("secret.png")
    // Minimal 4-byte PNG header to make the file non-empty and valid-looking.
    try Data([0x89, 0x50, 0x4E, 0x47]).write(to: outsideImage)
    let symlinkInsideRoot = tempRoot.appendingPathComponent("escape.png")
    try fm.createSymbolicLink(at: symlinkInsideRoot, withDestinationURL: outsideImage)

    let result = MarkdownImageSourcePolicy.v1.resolve(
      source: symlinkInsideRoot.path,
      localRoots: [tempRoot]
    )
    #expect(result == nil, "Symlink escape to path outside allowed root must be rejected")
  }
}
