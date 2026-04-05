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

        #expect(blocks.count == 3)
        #expect(blocks[0].kind == .markdown)
        #expect(blocks[2].kind == .markdown)

        guard case let .image(imageBlock) = blocks[1] else {
            Issue.record("Expected middle block to be an image block")
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
}
