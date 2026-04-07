import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 027", .serialized)
struct Proposal027Tests {
    private let container: ModelContainer
    private let context: ModelContext
    private let tempDirectory: URL

    init() throws {
        let (container, context) = try makeTestModelContainer()
        self.container = container
        self.context = context
        self.tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("Proposal027Tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
    }

    @Test("Artifact-backed render context preserves canonical artifact format and local roots")
    func artifactBackedRenderContextPreservesCanonicalFormat() throws {
        let workspace = makeTestWorkspace(tempDir: tempDirectory)
        let run = try makeTestRun(workspace: workspace, context: context)
        let artifact = Artifact(
            name: "run_report_v4",
            contractID: "run_report",
            format: .report,
            filePath: workspace.artifactRoot.appendingPathComponent("reports/run_report_v4.md").path,
            runID: run.id,
            stageID: "state_7_implementation_started",
            agentID: "lead_orchestrator",
            provider: "claude-code"
        )

        let renderContext = ArtifactRenderContext.artifactBacked(artifact: artifact, run: run)

        #expect(renderContext.format == .report)
        #expect(renderContext.localRoots.contains(URL(fileURLWithPath: run.artifactRoot, isDirectory: true)))
        #expect(renderContext.localRoots.contains(URL(fileURLWithPath: run.workspaceRoot, isDirectory: true)))
        #expect(renderContext.localRoots.contains(URL(fileURLWithPath: artifact.filePath).deletingLastPathComponent()))
    }

    @Test("Explicit render context is available only for non-artifact content")
    func explicitRenderContextSupportsNonArtifactContent() {
        let renderContext = ArtifactRenderContext.explicit(format: .markdown)

        #expect(renderContext.format == .markdown)
        #expect(renderContext.localRoots.isEmpty)
        #expect(renderContext.provenance == .explicit)
    }

    @Test("Markdown image source policy resolves local absolute and workspace-relative sources")
    func markdownImageSourcePolicyResolvesLocalSources() throws {
        let workspaceRoot = tempDirectory.appendingPathComponent("workspace", isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        let imagesDir = workspaceRoot.appendingPathComponent("docs/images", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: imagesDir, withIntermediateDirectories: true)

        let absoluteImage = imagesDir.appendingPathComponent("diagram.png")
        FileManager.default.createFile(atPath: absoluteImage.path, contents: Data("png".utf8))

        let allowedRoots = [artifactRoot, workspaceRoot]

        let absoluteURL = MarkdownImageSourcePolicy.v1.resolve(source: absoluteImage.path, localRoots: allowedRoots)
        let relativeURL = MarkdownImageSourcePolicy.v1.resolve(source: "docs/images/diagram.png", localRoots: allowedRoots)

        #expect(absoluteURL == absoluteImage.standardizedFileURL)
        #expect(relativeURL == absoluteImage.standardizedFileURL)
    }

    @Test("Markdown image source policy rejects remote and out-of-bound sources")
    func markdownImageSourcePolicyRejectsUnsafeSources() throws {
        let workspaceRoot = tempDirectory.appendingPathComponent("workspace-safe", isDirectory: true)
        let artifactRoot = workspaceRoot.appendingPathComponent("artifacts", isDirectory: true)
        let outsideRoot = tempDirectory.appendingPathComponent("outside", isDirectory: true)
        try FileManager.default.createDirectory(at: artifactRoot, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: outsideRoot, withIntermediateDirectories: true)

        let outsideImage = outsideRoot.appendingPathComponent("diagram.png")
        FileManager.default.createFile(atPath: outsideImage.path, contents: Data("png".utf8))

        let allowedRoots = [artifactRoot, workspaceRoot]

        #expect(MarkdownImageSourcePolicy.v1.resolve(source: "https://example.com/diagram.png", localRoots: allowedRoots) == nil)
        #expect(MarkdownImageSourcePolicy.v1.resolve(source: outsideImage.path, localRoots: allowedRoots) == nil)
        #expect(MarkdownImageSourcePolicy.v1.resolve(source: "../outside/diagram.png", localRoots: allowedRoots) == nil)
    }

    @Test("Markdown document parser extracts standalone image blocks and keeps prose blocks")
    func markdownDocumentParserPartitionsStandaloneImages() throws {
        let workspaceRoot = tempDirectory.appendingPathComponent("workspace-markdown", isDirectory: true)
        let docsDir = workspaceRoot.appendingPathComponent("docs/images", isDirectory: true)
        try FileManager.default.createDirectory(at: docsDir, withIntermediateDirectories: true)
        let imageURL = docsDir.appendingPathComponent("chart.png")
        FileManager.default.createFile(atPath: imageURL.path, contents: Data("png".utf8))

        let blocks = MarkdownDocumentParser.parse(
            """
            # Title

            Intro paragraph.

            ![Chart](docs/images/chart.png)

            Outro paragraph.
            """,
            localRoots: [workspaceRoot]
        )

        #expect(blocks.count == 4)
        #expect(blocks[0].kind == MarkdownDocumentBlockKind.heading)
        #expect(blocks[1].kind == MarkdownDocumentBlockKind.paragraph)
        #expect(blocks[3].kind == MarkdownDocumentBlockKind.paragraph)

        guard case let .image(imageBlock) = blocks[2] else {
            Issue.record("Expected third block to be an image block")
            return
        }

        #expect(imageBlock.altText == "Chart")
        #expect(imageBlock.resolvedURL == imageURL.standardizedFileURL)
        #expect(imageBlock.isAllowed)
    }

    @Test("JSON tree parser builds object and array summaries")
    func jsonTreeParserBuildsStructuredSummaries() throws {
        let root = try JSONTreeNode.parse(
            """
            {
              "workflow": "Full MVP Live",
              "agents": [
                { "id": "proposal_writer", "enabled": true }
              ],
              "count": 1
            }
            """
        )

        guard case let .object(entries) = root.kind else {
            Issue.record("Expected object root node")
            return
        }

        #expect(root.collapsedSummary == "{3 keys}")
        #expect(entries.map(\.key) == ["agents", "count", "workflow"])

        guard let agentsEntry = entries.first(where: { $0.key == "agents" }) else {
            Issue.record("Expected agents entry")
            return
        }

        #expect(agentsEntry.node.collapsedSummary == "[1 item]")
    }

    @Test("Markdown-declared JSON payload is rescued into structured JSON presentation")
    func markdownDeclaredJSONPayloadRescuesToJSONPresentation() {
        let intent = ArtifactPresentationIntent.resolve(
            content: """
            {
              "summary": "Structured payload",
              "count": 2
            }
            """,
            context: .explicit(format: .markdown)
        )

        #expect(intent == .jsonTree(rescuedFrom: .markdown))
    }

    @Test("Report-declared JSON payload is rescued into structured JSON presentation")
    func reportDeclaredJSONPayloadRescuesToJSONPresentation() {
        let intent = ArtifactPresentationIntent.resolve(
            content: """
            [
              { "id": "a" },
              { "id": "b" }
            ]
            """,
            context: .explicit(format: .report)
        )

        #expect(intent == .jsonTree(rescuedFrom: .report))
    }

    @Test("Workflow run artifact snapshot derives approval and debug slices from provided artifacts")
    func workflowRunArtifactSnapshotDerivesSlicesWithoutFetching() throws {
        let workspace = makeTestWorkspace(tempDir: tempDirectory)
        let run = try makeTestRun(workspace: workspace, context: context)
        let now = Date()

        let artifacts = [
            Artifact(
                name: "proposal_review_summary",
                contractID: "proposal_review_summary",
                format: .json,
                filePath: workspace.artifactRoot.appendingPathComponent("proposal_review_summary.json").path,
                createdAt: now.addingTimeInterval(-20),
                runID: run.id,
                stageID: "state_3_proposal_reviewed",
                agentID: "lead_orchestrator",
                provider: "claude-code"
            ),
            Artifact(
                name: "proposal_current",
                contractID: "proposal_current",
                format: .markdown,
                filePath: workspace.artifactRoot.appendingPathComponent("proposal_current.md").path,
                createdAt: now.addingTimeInterval(-10),
                runID: run.id,
                stageID: "state_2_proposal_drafted",
                agentID: "proposal_writer",
                provider: "codex"
            ),
            Artifact(
                name: "proposal_writer_transcript.md",
                contractID: "proposal_writer_transcript",
                format: .markdown,
                filePath: workspace.artifactRoot.appendingPathComponent("proposal_writer_transcript.md").path,
                createdAt: now.addingTimeInterval(-5),
                runID: run.id,
                stageID: "state_2_proposal_drafted",
                agentID: "proposal_writer",
                provider: "codex"
            ),
            Artifact(
                name: "proposal_writer_receipt.json",
                contractID: "proposal_writer_receipt",
                format: .json,
                filePath: workspace.artifactRoot.appendingPathComponent("proposal_writer_receipt.json").path,
                createdAt: now.addingTimeInterval(-4),
                runID: run.id,
                stageID: "state_2_proposal_drafted",
                agentID: "proposal_writer",
                provider: "codex"
            ),
            Artifact(
                name: "final_feature_report",
                contractID: "final_feature_report",
                format: .markdown,
                filePath: workspace.artifactRoot.appendingPathComponent("final_feature_report.md").path,
                createdAt: now.addingTimeInterval(-100),
                runID: run.id,
                stageID: "state_9_complete",
                agentID: "lead_orchestrator",
                provider: "claude-code"
            )
        ]

        let snapshot = WorkflowRunArtifactSnapshot(artifacts: artifacts)

        #expect(snapshot.latestArtifacts.first?.name == "final_feature_report")
        #expect(snapshot.approvalContextArtifacts.map(\.name) == [
            "proposal_review_summary",
            "proposal_current"
        ])
        #expect(snapshot.latestDebugArtifacts.map(\.name) == [
            "proposal_writer_receipt.json",
            "proposal_writer_transcript.md"
        ])
    }

    @Test("Native markdown keeps markdown presentation intent when payload is not JSON")
    func nativeMarkdownKeepsMarkdownPresentationIntent() {
        let intent = ArtifactPresentationIntent.resolve(
            content: """
            # Proposal Summary

            This is prose, not JSON.
            """,
            context: .explicit(format: .markdown)
        )

        #expect(intent == .markdownDocument)
    }

    @Test("Transcript markdown remains a markdown document presentation")
    func transcriptMarkdownRemainsMarkdownPresentation() {
        let intent = ArtifactPresentationIntent.resolve(
            content: """
            Execution Transcript

            Long streaming output line.
            """,
            context: .explicitNamed(format: .markdown, artifactName: "proposal_writer_transcript.md")
        )

        #expect(intent == .markdownDocument)
    }

    @Test("Large markdown remains a markdown document presentation")
    func largeMarkdownRemainsMarkdownPresentation() {
        let largeMarkdown = String(repeating: "Long markdown content line\n", count: 1_500)
        let intent = ArtifactPresentationIntent.resolve(
            content: largeMarkdown,
            context: .explicit(format: .markdown)
        )

        #expect(intent == .markdownDocument)
    }

    @Test("Markdown document parser classifies structural blocks")
    func markdownDocumentParserClassifiesStructuralBlocks() throws {
        let workspaceRoot = tempDirectory.appendingPathComponent("workspace-structure", isDirectory: true)
        let docsDir = workspaceRoot.appendingPathComponent("docs/images", isDirectory: true)
        try FileManager.default.createDirectory(at: docsDir, withIntermediateDirectories: true)
        let imageURL = docsDir.appendingPathComponent("chart.png")
        FileManager.default.createFile(atPath: imageURL.path, contents: Data("png".utf8))

        let blocks = MarkdownDocumentParser.parse(
            """
            # Heading

            Intro paragraph with **bold**.

            - Item one
            - Item two

            > Quote line

            ```swift
            let x = 1
            ```

            | A | B |
            |---|---|
            | 1 | 2 |

            ![Chart](docs/images/chart.png)
            """,
            localRoots: [workspaceRoot]
        )

        #expect(blocks.count == 7)
        #expect(blocks[0].kind == MarkdownDocumentBlockKind.heading)
        #expect(blocks[1].kind == MarkdownDocumentBlockKind.paragraph)
        #expect(blocks[2].kind == MarkdownDocumentBlockKind.list)
        #expect(blocks[3].kind == MarkdownDocumentBlockKind.blockQuote)
        #expect(blocks[4].kind == MarkdownDocumentBlockKind.codeBlock)
        #expect(blocks[5].kind == MarkdownDocumentBlockKind.table)
        #expect(blocks[6].kind == MarkdownDocumentBlockKind.image)

        guard case let .image(imageBlock) = blocks[6] else {
            Issue.record("Expected final block to be an image block")
            return
        }

        #expect(imageBlock.resolvedURL == imageURL.standardizedFileURL)
    }

    @Test("Markdown document loader can prepare large markdown blocks off the view body path")
    func markdownDocumentLoaderPreparesLargeMarkdownBlocks() async {
        let largeMarkdown = String(
            repeating: """
            # Run Report

            - item
            - item
            - item

            Paragraph content here.

            """,
            count: 500
        )

        let blocks = await MarkdownDocumentLoader.load(content: largeMarkdown, localRoots: [])

        #expect(!blocks.isEmpty)
        #expect(blocks.contains(where: { $0.kind == .heading }))
        #expect(blocks.contains(where: { $0.kind == .list }))
        #expect(blocks.contains(where: { $0.kind == .paragraph }))
    }

    @Test("Artifact inspector compacts multi-line skill summaries into a single diagnostic line")
    func artifactInspectorCompactsSkillSummary() {
        let compacted = ArtifactInspectorSkillTruthFormatter.compactSummary(
            """
            ---
            name: proposal-review-triad

            description: Review repo-local proposals.

            Use architecture-only mode.
            """
        )

        #expect(compacted == "--- name: proposal-review-triad description: Review repo-local proposals. Use architecture-only mode.")
    }

    @Test("Artifact inspector truncates oversized skill summary diagnostics")
    func artifactInspectorTruncatesOversizedSkillSummary() throws {
        let oversized = Array(repeating: "proposal-review-triad diagnostic payload", count: 20).joined(separator: "\n")
        let compacted = try #require(ArtifactInspectorSkillTruthFormatter.compactSummary(oversized))

        #expect(compacted.hasSuffix("…"))
        #expect(compacted.count <= 221)
    }

    @Test("Timeline error presentation summarizes verbose runtime errors")
    func timelineErrorPresentationSummarizesVerboseRuntimeErrors() {
        let raw = """
        Request failed: Gemini CLI command failed (exit code Some(1): [WARN] Skipping unreadable directory: /Library/Bluetooth (EPERM: operation not permitted, scandir '/Library/Bluetooth')
        FATAL ERROR: Ineffective mark-compacts near heap limit Allocation failed - JavaScript heap out of memory
        """

        let presentation = TimelineErrorPresentation(rawDetail: raw)

        #expect(presentation.summary == "Gemini CLI failed: JavaScript heap out of memory.")
        #expect(presentation.highlights.contains(where: { $0.contains("/Library/Bluetooth") }))
        #expect(presentation.highlights.contains(where: { $0.localizedCaseInsensitiveContains("out of memory") }))
        #expect(presentation.shouldOfferRawDisclosure)
    }
}
