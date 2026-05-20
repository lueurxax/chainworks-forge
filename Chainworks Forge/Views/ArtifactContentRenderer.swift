import SwiftUI
import Foundation
import AppKit
import SwiftData

nonisolated enum ArtifactRenderProvenance: Equatable {
    case artifactBacked
    case explicit
}

nonisolated struct ArtifactRenderContext: Equatable {
    let format: ArtifactFormat
    let artifactName: String?
    let localRoots: [URL]
    let provenance: ArtifactRenderProvenance

    static func artifactBacked(artifact: Artifact, run: Run? = nil) -> ArtifactRenderContext {
        var roots: [URL] = [
            URL(fileURLWithPath: artifact.filePath).deletingLastPathComponent()
        ]
        if let run {
            if !run.artifactRoot.isEmpty {
                roots.append(URL(fileURLWithPath: run.artifactRoot, isDirectory: true))
            }
            if !run.workspaceRoot.isEmpty {
                roots.append(URL(fileURLWithPath: run.workspaceRoot, isDirectory: true))
            }
        }
        return ArtifactRenderContext(
            format: artifact.format,
            artifactName: artifact.name,
            localRoots: deduplicatedRoots(roots),
            provenance: .artifactBacked
        )
    }

    static func explicit(format: ArtifactFormat, localRoots: [URL] = []) -> ArtifactRenderContext {
        ArtifactRenderContext(
            format: format,
            artifactName: nil,
            localRoots: deduplicatedRoots(localRoots),
            provenance: .explicit
        )
    }

    static func explicitNamed(format: ArtifactFormat, artifactName: String?, localRoots: [URL] = []) -> ArtifactRenderContext {
        ArtifactRenderContext(
            format: format,
            artifactName: artifactName,
            localRoots: deduplicatedRoots(localRoots),
            provenance: .explicit
        )
    }

    private static func deduplicatedRoots(_ roots: [URL]) -> [URL] {
        var seen: Set<String> = []
        return roots.compactMap { root in
            let standardized = root.standardizedFileURL
            let key = standardized.path
            guard !key.isEmpty, seen.insert(key).inserted else { return nil }
            return standardized
        }
    }
}

nonisolated enum MarkdownImageSourcePolicy {
    case v1

    private static let allowedRenderableExtensions: Set<String> = [
        "png", "jpg", "jpeg", "gif", "webp",
        "bmp", "tif", "tiff", "heic", "heif", "pdf"
    ]

    private static let blockedAncestorExtensions: Set<String> = [
        "photoslibrary", "photolibrary",
        "app", "appex", "bundle", "framework",
        "plugin", "pkg", "xcodeproj", "xcworkspace", "playground"
    ]

    func resolve(source: String, localRoots: [URL]) -> URL? {
        let trimmed = source.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        guard !trimmed.split(separator: "/").contains("..") else { return nil }

        if let url = URL(string: trimmed), let scheme = url.scheme?.lowercased(), !scheme.isEmpty {
            guard scheme == "file", url.isFileURL else { return nil }
            return resolveLocalURL(url, localRoots: localRoots)
        }

        let candidate = URL(fileURLWithPath: trimmed)
        if trimmed.hasPrefix("/") {
            return resolveLocalURL(candidate, localRoots: localRoots)
        }

        // Route through the full symlink-safe check to prevent escape via symlinked ancestor dirs.
        for root in localRoots {
            let candidate = root.appendingPathComponent(trimmed).standardizedFileURL
            if let safe = resolveLocalURL(candidate, localRoots: localRoots) {
                return safe
            }
        }
        return nil
    }

    private func resolveLocalURL(_ url: URL, localRoots: [URL]) -> URL? {
        let standardized = url.standardizedFileURL
        guard isAllowed(standardized, within: localRoots) else { return nil }
        // Resolve symlinks and re-check against canonical roots to prevent symlink escape.
        let resolved = standardized.resolvingSymlinksInPath()
        let canonicalRoots = localRoots.map { $0.resolvingSymlinksInPath() }
        let resolvedPath = resolved.path
        let symlinkSafe = canonicalRoots.contains { root in
            let rootPath = root.path
            return resolvedPath == rootPath || resolvedPath.hasPrefix(rootPath + "/")
        }
        guard symlinkSafe else { return nil }
        return isSafeRenderableLocalFile(standardized) ? standardized : nil
    }

    private func isAllowed(_ url: URL, within roots: [URL]) -> Bool {
        let path = url.standardizedFileURL.path
        return roots.contains { root in
            let rootPath = root.standardizedFileURL.path
            return path == rootPath || path.hasPrefix(rootPath + "/")
        }
    }

    private func isSafeRenderableLocalFile(_ url: URL) -> Bool {
        let standardized = url.standardizedFileURL
        let path = standardized.path
        guard !path.isEmpty else { return false }

        let lowercasedComponents = standardized.pathComponents.map { $0.lowercased() }
        if lowercasedComponents.contains(where: { component in
            guard let ext = component.split(separator: ".").last.map(String.init),
                  component.contains(".")
            else { return false }
            return Self.blockedAncestorExtensions.contains(ext)
        }) {
            return false
        }

        let fileExtension = standardized.pathExtension.lowercased()
        guard Self.allowedRenderableExtensions.contains(fileExtension) else { return false }

        do {
            let resourceValues = try standardized.resourceValues(forKeys: [.isRegularFileKey, .isPackageKey])
            guard resourceValues.isPackage != true else { return false }
            return resourceValues.isRegularFile == true
        } catch {
            return false
        }
    }
}

enum MarkdownDocumentBlockKind: Equatable {
    case heading
    case paragraph
    case list
    case blockQuote
    case codeBlock
    case table
    case image
}

struct MarkdownImageBlock: Equatable {
    let altText: String
    let source: String
    let resolvedURL: URL?
    let isAllowed: Bool
}

struct MarkdownListItem: Equatable {
    let ordinal: Int?
    let text: String
}

struct MarkdownTableBlock: Equatable {
    let header: [String]
    let rows: [[String]]
}

enum MarkdownDocumentBlock: Equatable {
    case heading(level: Int, text: String)
    case paragraph(String)
    case list(items: [MarkdownListItem], ordered: Bool)
    case blockQuote(String)
    case codeBlock(language: String?, code: String)
    case table(MarkdownTableBlock)
    case image(MarkdownImageBlock)

    var kind: MarkdownDocumentBlockKind {
        switch self {
        case .heading: return .heading
        case .paragraph: return .paragraph
        case .list: return .list
        case .blockQuote: return .blockQuote
        case .codeBlock: return .codeBlock
        case .table: return .table
        case .image: return .image
        }
    }
}

nonisolated enum MarkdownDocumentParser {
    private static let imagePattern = try! NSRegularExpression(
        pattern: #"^\s*!\[([^\]]*)\]\(([^)]+)\)\s*$"#,
        options: []
    )
    private static let headingPattern = try! NSRegularExpression(
        pattern: #"^\s*(#{1,6})\s+(.+?)\s*$"#,
        options: []
    )
    private static let unorderedListPattern = try! NSRegularExpression(
        pattern: #"^\s*[-*+]\s+(.+?)\s*$"#,
        options: []
    )
    private static let orderedListPattern = try! NSRegularExpression(
        pattern: #"^\s*(\d+)\.\s+(.+?)\s*$"#,
        options: []
    )

    nonisolated static func parse(_ content: String, localRoots: [URL], policy: MarkdownImageSourcePolicy = .v1) -> [MarkdownDocumentBlock] {
        let normalized = content.replacingOccurrences(of: "\r\n", with: "\n")
        var blocks: [MarkdownDocumentBlock] = []
        let lines = normalized.components(separatedBy: .newlines)
        var index = 0

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)

            if trimmed.isEmpty {
                index += 1
                continue
            }

            if let imageBlock = parseStandaloneImage(trimmed, localRoots: localRoots, policy: policy) {
                blocks.append(.image(imageBlock))
                index += 1
                continue
            }

            if let heading = parseHeading(trimmed) {
                blocks.append(heading)
                index += 1
                continue
            }

            if let codeBlock = parseCodeBlock(lines: lines, startingAt: index) {
                blocks.append(.codeBlock(language: codeBlock.language, code: codeBlock.code))
                index = codeBlock.nextIndex
                continue
            }

            if let tableBlock = parseTable(lines: lines, startingAt: index) {
                blocks.append(.table(tableBlock.table))
                index = tableBlock.nextIndex
                continue
            }

            if let listBlock = parseList(lines: lines, startingAt: index) {
                blocks.append(.list(items: listBlock.items, ordered: listBlock.ordered))
                index = listBlock.nextIndex
                continue
            }

            if let quoteBlock = parseBlockQuote(lines: lines, startingAt: index) {
                blocks.append(.blockQuote(quoteBlock.text))
                index = quoteBlock.nextIndex
                continue
            }

            let paragraph = parseParagraph(lines: lines, startingAt: index)
            blocks.append(.paragraph(paragraph.text))
            index = paragraph.nextIndex
        }

        if blocks.isEmpty, !content.isEmpty {
            blocks.append(.paragraph(content))
        }

        return blocks
    }

    private static func parseStandaloneImage(
        _ paragraph: String,
        localRoots: [URL],
        policy: MarkdownImageSourcePolicy
    ) -> MarkdownImageBlock? {
        let range = NSRange(paragraph.startIndex..<paragraph.endIndex, in: paragraph)
        guard let match = imagePattern.firstMatch(in: paragraph, options: [], range: range),
              let altRange = Range(match.range(at: 1), in: paragraph),
              let sourceRange = Range(match.range(at: 2), in: paragraph)
        else {
            return nil
        }

        let altText = String(paragraph[altRange])
        let source = String(paragraph[sourceRange])
        let resolvedURL = policy.resolve(source: source, localRoots: localRoots)
        return MarkdownImageBlock(
            altText: altText,
            source: source,
            resolvedURL: resolvedURL,
            isAllowed: resolvedURL != nil
        )
    }

    private static func parseHeading(_ line: String) -> MarkdownDocumentBlock? {
        let range = NSRange(line.startIndex..<line.endIndex, in: line)
        guard let match = headingPattern.firstMatch(in: line, options: [], range: range),
              let levelRange = Range(match.range(at: 1), in: line),
              let textRange = Range(match.range(at: 2), in: line)
        else {
            return nil
        }
        let level = line[levelRange].count
        let text = String(line[textRange]).trimmingCharacters(in: .whitespacesAndNewlines)
        return .heading(level: level, text: text)
    }

    private static func parseCodeBlock(lines: [String], startingAt index: Int) -> (language: String?, code: String, nextIndex: Int)? {
        let opener = lines[index].trimmingCharacters(in: .whitespaces)
        guard opener.hasPrefix("```") else { return nil }

        let languageHint = opener.dropFirst(3).trimmingCharacters(in: .whitespacesAndNewlines)
        var cursor = index + 1
        var codeLines: [String] = []

        while cursor < lines.count {
            let line = lines[cursor]
            if line.trimmingCharacters(in: .whitespaces) == "```" {
                return (
                    language: languageHint.isEmpty ? nil : languageHint,
                    code: codeLines.joined(separator: "\n"),
                    nextIndex: cursor + 1
                )
            }
            codeLines.append(line)
            cursor += 1
        }

        return (
            language: languageHint.isEmpty ? nil : languageHint,
            code: codeLines.joined(separator: "\n"),
            nextIndex: cursor
        )
    }

    private static func parseTable(lines: [String], startingAt index: Int) -> (table: MarkdownTableBlock, nextIndex: Int)? {
        guard index + 1 < lines.count else { return nil }
        let header = splitTableRow(lines[index])
        guard header.count >= 2, isTableSeparator(lines[index + 1]) else { return nil }

        var rows: [[String]] = []
        var cursor = index + 2
        while cursor < lines.count {
            let row = splitTableRow(lines[cursor])
            guard row.count >= 2 else { break }
            rows.append(row)
            cursor += 1
        }

        return (MarkdownTableBlock(header: header, rows: rows), cursor)
    }

    private static func parseList(lines: [String], startingAt index: Int) -> (items: [MarkdownListItem], ordered: Bool, nextIndex: Int)? {
        guard let firstMatch = parseListItem(lines[index]) else { return nil }

        var items: [MarkdownListItem] = [firstMatch.item]
        let ordered = firstMatch.ordered
        var cursor = index + 1

        while cursor < lines.count {
            let line = lines[cursor]
            if line.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { break }

            if let match = parseListItem(line), match.ordered == ordered {
                items.append(match.item)
                cursor += 1
                continue
            }

            guard isListContinuation(line) else { break }
            if let last = items.indices.last {
                let continuation = line.trimmingCharacters(in: .whitespacesAndNewlines)
                items[last] = MarkdownListItem(
                    ordinal: items[last].ordinal,
                    text: items[last].text + " " + continuation
                )
                cursor += 1
                continue
            }
            break
        }

        return (items, ordered, cursor)
    }

    private static func parseListItem(_ line: String) -> (item: MarkdownListItem, ordered: Bool)? {
        let range = NSRange(line.startIndex..<line.endIndex, in: line)
        if let match = orderedListPattern.firstMatch(in: line, options: [], range: range),
           let ordinalRange = Range(match.range(at: 1), in: line),
           let textRange = Range(match.range(at: 2), in: line),
           let ordinal = Int(line[ordinalRange]) {
            return (
                MarkdownListItem(
                    ordinal: ordinal,
                    text: String(line[textRange]).trimmingCharacters(in: .whitespacesAndNewlines)
                ),
                true
            )
        }

        if let match = unorderedListPattern.firstMatch(in: line, options: [], range: range),
           let textRange = Range(match.range(at: 1), in: line) {
            return (
                MarkdownListItem(
                    ordinal: nil,
                    text: String(line[textRange]).trimmingCharacters(in: .whitespacesAndNewlines)
                ),
                false
            )
        }

        return nil
    }

    private static func parseBlockQuote(lines: [String], startingAt index: Int) -> (text: String, nextIndex: Int)? {
        guard lines[index].trimmingCharacters(in: .whitespaces).hasPrefix(">") else { return nil }

        var quoteLines: [String] = []
        var cursor = index
        while cursor < lines.count {
            let trimmed = lines[cursor].trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix(">") else { break }
            let stripped = trimmed.dropFirst().trimmingCharacters(in: .whitespaces)
            quoteLines.append(stripped)
            cursor += 1
        }

        return (quoteLines.joined(separator: "\n"), cursor)
    }

    private static func parseParagraph(lines: [String], startingAt index: Int) -> (text: String, nextIndex: Int) {
        var paragraphLines: [String] = []
        var cursor = index

        while cursor < lines.count {
            let line = lines[cursor]
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty || isStructuralBoundary(lines: lines, at: cursor) {
                break
            }
            paragraphLines.append(trimmed)
            cursor += 1
        }

        return (paragraphLines.joined(separator: " "), cursor)
    }

    private static func isStructuralBoundary(lines: [String], at index: Int) -> Bool {
        let trimmed = lines[index].trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return true }
        if parseStandaloneImage(trimmed, localRoots: [], policy: .v1) != nil { return true }
        if parseHeading(trimmed) != nil { return true }
        if lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```") { return true }
        if parseListItem(lines[index]) != nil { return true }
        if lines[index].trimmingCharacters(in: .whitespaces).hasPrefix(">") { return true }
        if index + 1 < lines.count, splitTableRow(lines[index]).count >= 2, isTableSeparator(lines[index + 1]) {
            return true
        }
        return false
    }

    private static func isListContinuation(_ line: String) -> Bool {
        let trimmed = line.trimmingCharacters(in: .newlines)
        guard !trimmed.isEmpty else { return false }
        return line.hasPrefix(" ") || line.hasPrefix("\t")
    }

    private static func splitTableRow(_ line: String) -> [String] {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.contains("|") else { return [] }
        let withoutEdges = trimmed.trimmingCharacters(in: CharacterSet(charactersIn: "|"))
        return withoutEdges
            .split(separator: "|", omittingEmptySubsequences: false)
            .map { $0.trimmingCharacters(in: .whitespaces) }
    }

    private static func isTableSeparator(_ line: String) -> Bool {
        let cells = splitTableRow(line)
        guard !cells.isEmpty else { return false }
        return cells.allSatisfy { cell in
            let stripped = cell.replacingOccurrences(of: ":", with: "")
            return !stripped.isEmpty && stripped.allSatisfy { $0 == "-" }
        }
    }
}

struct JSONTreeEntry: Equatable {
    let key: String
    let node: JSONTreeNode
}

struct JSONTreeNode: Equatable {
    enum Kind: Equatable {
        case object([JSONTreeEntry])
        case array([JSONTreeNode])
        case string(String)
        case number(String)
        case boolean(Bool)
        case null
    }

    let path: String
    let kind: Kind

    var collapsedSummary: String {
        switch kind {
        case let .object(entries):
            return "{\(entries.count) \(entries.count == 1 ? "key" : "keys")}"
        case let .array(elements):
            return "[\(elements.count) \(elements.count == 1 ? "item" : "items")]"
        case let .string(value):
            return "\"\(value)\""
        case let .number(value):
            return value
        case let .boolean(value):
            return value ? "true" : "false"
        case .null:
            return "null"
        }
    }

    var isContainer: Bool {
        switch kind {
        case .object, .array:
            return true
        case .string, .number, .boolean, .null:
            return false
        }
    }

    static func parse(_ raw: String) throws -> JSONTreeNode {
        let data = Data(raw.utf8)
        let jsonObject = try JSONSerialization.jsonObject(with: data)
        return try build(jsonObject, path: "$")
    }

    func expansionSeed(maxDepth: Int, depth: Int = 0) -> Set<String> {
        guard isContainer, depth <= maxDepth else { return [] }
        var paths: Set<String> = [path]
        switch kind {
        case let .object(entries):
            for entry in entries {
                paths.formUnion(entry.node.expansionSeed(maxDepth: maxDepth, depth: depth + 1))
            }
        case let .array(elements):
            for element in elements {
                paths.formUnion(element.expansionSeed(maxDepth: maxDepth, depth: depth + 1))
            }
        case .string, .number, .boolean, .null:
            break
        }
        return paths
    }

    private static func build(_ value: Any, path: String) throws -> JSONTreeNode {
        switch value {
        case let dictionary as [String: Any]:
            let entries = try dictionary.keys.sorted().map { key in
                JSONTreeEntry(
                    key: key,
                    node: try build(dictionary[key] as Any, path: "\(path).\(key)")
                )
            }
            return JSONTreeNode(path: path, kind: .object(entries))
        case let array as [Any]:
            let elements = try array.enumerated().map { index, element in
                try build(element, path: "\(path)[\(index)]")
            }
            return JSONTreeNode(path: path, kind: .array(elements))
        case let string as String:
            return JSONTreeNode(path: path, kind: .string(string))
        case let number as NSNumber:
            if CFGetTypeID(number) == CFBooleanGetTypeID() {
                return JSONTreeNode(path: path, kind: .boolean(number.boolValue))
            }
            return JSONTreeNode(path: path, kind: .number(number.stringValue))
        case _ as NSNull:
            return JSONTreeNode(path: path, kind: .null)
        default:
            throw NSError(domain: "JSONTreeNode", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unsupported JSON value"])
        }
    }
}

struct ArtifactContentRenderer: View {
    let content: String
    let context: ArtifactRenderContext
    private let preparedPreview: ArtifactPreparedPreview?

    init(content: String, context: ArtifactRenderContext) {
        self.content = content
        self.context = context
        self.preparedPreview = nil
    }

    init(preparedPreview: ArtifactPreparedPreview, context: ArtifactRenderContext) {
        self.content = preparedPreview.content
        self.context = context
        self.preparedPreview = preparedPreview
    }

    var body: some View {
        let prepared = preparedPreview ?? {
            let intent = ArtifactPresentationIntent.resolve(content: content, context: context)
            return ArtifactPreviewPolicy.prepare(content: content, intent: intent)
        }()

        VStack(alignment: .leading, spacing: 12) {
            if let previewNotice = prepared.previewNotice {
                ArtifactPreviewNoticeView(notice: previewNotice)
            }

            switch prepared.intent {
            case .markdownDocument:
                MarkdownDocumentView(content: prepared.content, localRoots: context.localRoots)
            case .jsonTree:
                if context.artifactName == "proposal_review_summary" {
                    ProposalReviewSummaryArtifactView(rawJSON: prepared.content)
                } else {
                    JSONTreeDocumentView(rawJSON: prepared.content)
                }
            case .diff:
                DiffArtifactView(content: prepared.content)
            case .plainText(let monospaced):
                PlainTextArtifactView(content: prepared.content, monospaced: monospaced)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

nonisolated enum ArtifactPresentationIntent: Equatable, Sendable {
    case markdownDocument
    case jsonTree(rescuedFrom: ArtifactFormat?)
    case diff
    case plainText(monospaced: Bool)

    static func resolve(content: String, context: ArtifactRenderContext) -> ArtifactPresentationIntent {
        if let rescuedFrom = rescuedJSONSourceFormat(content: content, declaredFormat: context.format) {
            return .jsonTree(rescuedFrom: rescuedFrom)
        }

        switch context.format {
        case .markdown:
            return .markdownDocument
        case .json:
            if StructuredPayloadProbe.isTopLevelJSONObjectOrArray(content) {
                return .jsonTree(rescuedFrom: nil)
            }
            if StructuredPayloadProbe.looksLikeDiff(content) {
                return .diff
            }
            if StructuredPayloadProbe.looksLikeMarkdown(content) {
                return .markdownDocument
            }
            return .plainText(monospaced: true)
        case .diff:
            return .diff
        case .report:
            return .plainText(monospaced: true)
        }
    }

    private static func rescuedJSONSourceFormat(content: String, declaredFormat: ArtifactFormat) -> ArtifactFormat? {
        guard declaredFormat == .markdown || declaredFormat == .report else { return nil }
        guard StructuredPayloadProbe.isTopLevelJSONObjectOrArray(content) else { return nil }
        return declaredFormat
    }
}

nonisolated struct ArtifactPreparedPreview: Equatable, Sendable {
    let content: String
    let intent: ArtifactPresentationIntent
    let previewNotice: ArtifactPreviewNotice?
}

nonisolated struct ArtifactPreviewNotice: Equatable, Sendable {
    let visibleCharacterCount: Int
    let totalCharacterCount: Int
    let visibleLineCount: Int
    let totalLineCount: Int
    let renderedAsRawText: Bool

    var message: String {
        let lineSummary = "\(visibleLineCount.formatted())/\(totalLineCount.formatted()) lines"
        let characterSummary = "\(visibleCharacterCount.formatted())/\(totalCharacterCount.formatted()) characters"
        if renderedAsRawText {
            return "Large artifact preview is capped and shown as raw text: \(lineSummary), \(characterSummary)."
        }
        return "Large artifact preview is capped: \(lineSummary), \(characterSummary)."
    }
}

nonisolated enum ArtifactPreviewPolicy {
    static let maxRenderedCharacters = 120_000
    static let maxRenderedLines = 2_000
    static let maxJSONTreeCharacters = 80_000

    static func prepare(content: String, intent: ArtifactPresentationIntent) -> ArtifactPreparedPreview {
        let forceRawText = shouldRenderAsRawText(content: content, intent: intent)
        let truncated = cappedPrefix(content)

        guard let truncated else {
            if forceRawText {
                return ArtifactPreparedPreview(
                    content: content,
                    intent: .plainText(monospaced: true),
                    previewNotice: nil
                )
            }
            return ArtifactPreparedPreview(content: content, intent: intent, previewNotice: nil)
        }

        return ArtifactPreparedPreview(
            content: truncated.content,
            intent: forceRawText ? .plainText(monospaced: true) : intent,
            previewNotice: ArtifactPreviewNotice(
                visibleCharacterCount: truncated.visibleCharacterCount,
                totalCharacterCount: truncated.totalCharacterCount,
                visibleLineCount: truncated.visibleLineCount,
                totalLineCount: truncated.totalLineCount,
                renderedAsRawText: forceRawText
            )
        )
    }

    private static func shouldRenderAsRawText(content: String, intent: ArtifactPresentationIntent) -> Bool {
        switch intent {
        case .jsonTree:
            return content.count > maxJSONTreeCharacters || lineCount(content) > maxRenderedLines
        case .markdownDocument, .diff, .plainText:
            return false
        }
    }

    private static func cappedPrefix(_ content: String) -> (
        content: String,
        visibleCharacterCount: Int,
        totalCharacterCount: Int,
        visibleLineCount: Int,
        totalLineCount: Int
    )? {
        var cursor = content.startIndex
        var visibleCharacters = 0
        var visibleLines = content.isEmpty ? 0 : 1

        while cursor < content.endIndex {
            if visibleCharacters >= maxRenderedCharacters || visibleLines > maxRenderedLines {
                break
            }

            let character = content[cursor]
            if character.isNewline, visibleLines == maxRenderedLines {
                break
            }

            visibleCharacters += 1
            if character.isNewline {
                visibleLines += 1
            }
            content.formIndex(after: &cursor)
        }

        guard cursor < content.endIndex else { return nil }

        return (
            content: String(content[..<cursor]),
            visibleCharacterCount: visibleCharacters,
            totalCharacterCount: content.count,
            visibleLineCount: visibleLines,
            totalLineCount: lineCount(content)
        )
    }

    private static func lineCount(_ content: String) -> Int {
        guard !content.isEmpty else { return 0 }
        return content.reduce(into: 1) { count, character in
            if character.isNewline {
                count += 1
            }
        }
    }
}

private struct ArtifactPreviewNoticeView: View {
    let notice: ArtifactPreviewNotice

    var body: some View {
        Label(notice.message, systemImage: "scissors")
            .font(.caption)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(Color.secondary.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .textSelection(.enabled)
    }
}

nonisolated enum StructuredPayloadProbe {
    nonisolated private static let maxSynchronousJSONParseBytes = 80_000
    nonisolated private static let maxProbeCharacters = 16_000

    static func isTopLevelJSONObjectOrArray(_ content: String) -> Bool {
        guard let first = firstNonWhitespace(in: content), first == "{" || first == "[" else {
            return false
        }
        guard let last = lastNonWhitespace(in: content) else { return false }
        if first == "{", last != "}" { return false }
        if first == "[", last != "]" { return false }

        if content.utf8.count > maxSynchronousJSONParseBytes {
            return true
        }

        guard let data = content.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data)
        else {
            return false
        }

        return object is [String: Any] || object is [Any]
    }

    static func looksLikeMarkdown(_ content: String) -> Bool {
        let normalized = sampledPrefix(content).replacingOccurrences(of: "\r\n", with: "\n")
        let lines = normalized.split(separator: "\n", omittingEmptySubsequences: false)
        var evidenceCount = 0

        for line in lines.prefix(40) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { continue }
            if trimmed.hasPrefix("# ") || trimmed.hasPrefix("## ") || trimmed.hasPrefix("### ") {
                return true
            }
            if trimmed.hasPrefix("```") || trimmed.hasPrefix("> ") {
                return true
            }
            if trimmed.hasPrefix("- ") || trimmed.hasPrefix("* ") {
                evidenceCount += 1
            }
            if trimmed.contains("](") || trimmed.contains("**") || trimmed.contains("`") {
                evidenceCount += 1
            }
            if trimmed.contains("|"), trimmed.filter({ $0 == "|" }).count >= 2 {
                evidenceCount += 1
            }
            if evidenceCount >= 2 {
                return true
            }
        }

        return false
    }

    static func looksLikeDiff(_ content: String) -> Bool {
        let lines = sampledPrefix(content).replacingOccurrences(of: "\r\n", with: "\n")
            .split(separator: "\n", omittingEmptySubsequences: false)
            .prefix(40)
            .map { $0.trimmingCharacters(in: .whitespaces) }
        return lines.contains { line in
            line.hasPrefix("diff --git ")
                || line.hasPrefix("@@ ")
                || line.hasPrefix("+++ ")
                || line.hasPrefix("--- ")
        }
    }

    private static func sampledPrefix(_ content: String) -> String {
        guard content.count > maxProbeCharacters else { return content }
        return String(content.prefix(maxProbeCharacters))
    }

    private static func firstNonWhitespace(in content: String) -> Character? {
        content.first { !$0.isWhitespace }
    }

    private static func lastNonWhitespace(in content: String) -> Character? {
        content.reversed().first { !$0.isWhitespace }
    }
}

struct MarkdownDocumentView: View {
    let content: String
    let localRoots: [URL]

    @State private var blocks: [MarkdownDocumentBlock] = []
    @State private var isLoadingBlocks = true

    var body: some View {
        Group {
            if isLoadingBlocks {
                VStack(alignment: .leading, spacing: 12) {
                    ForgeSkeleton.headline(width: 200)
                    ForgeSkeleton.text(width: nil)
                    ForgeSkeleton.text(width: nil)
                    ForgeSkeleton.text(width: 250)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            } else {
                LazyVStack(alignment: .leading, spacing: 18) {
                    ForEach(blocks.indices, id: \.self) { index in
                        MarkdownDocumentBlockView(block: blocks[index])
                            .id(index)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .task(id: renderTaskKey) {
            isLoadingBlocks = true
            blocks = await MarkdownDocumentLoader.load(content: content, localRoots: localRoots)
            isLoadingBlocks = false
        }
    }

    private var renderTaskKey: Int {
        var hasher = Hasher()
        hasher.combine(content)
        for root in localRoots {
            hasher.combine(root.path)
        }
        return hasher.finalize()
    }
}

enum MarkdownDocumentLoader {
    nonisolated static func load(content: String, localRoots: [URL]) async -> [MarkdownDocumentBlock] {
        await Task.detached(priority: .userInitiated) {
            MarkdownDocumentParser.parse(content, localRoots: localRoots)
        }.value
    }
}

private struct MarkdownDocumentBlockView: View {
    let block: MarkdownDocumentBlock

    var body: some View {
        switch block {
        case let .heading(level, text):
            MarkdownRichTextBlockView(content: text, role: .heading(level))
        case let .paragraph(text):
            MarkdownRichTextBlockView(content: text, role: .body)
        case let .list(items, ordered):
            MarkdownListBlockView(items: items, ordered: ordered)
        case let .blockQuote(text):
            MarkdownBlockQuoteView(text: text)
        case let .codeBlock(language, code):
            MarkdownCodeBlockView(language: language, code: code)
        case let .table(table):
            MarkdownTableView(table: table)
        case let .image(imageBlock):
            MarkdownImageBlockView(block: imageBlock)
        }
    }
}

private enum MarkdownTextRole {
    case heading(Int)
    case body
    case listItem
    case blockQuote
    case codeBlock
    case tableHeader
    case tableCell
}

private enum MarkdownAttributedStringBuilder {
    static func build(markdown: String, role: MarkdownTextRole) -> NSAttributedString {
        switch role {
        case .codeBlock:
            return codeBlockString(markdown)
        default:
            guard let attributed = try? AttributedString(
                markdown: markdown,
                options: .init(interpretedSyntax: .full)
            ) else {
                return NSAttributedString(
                    string: markdown,
                    attributes: baseAttributes(for: role)
                )
            }

            let mutable = NSMutableAttributedString()
            for run in attributed.runs {
                let text = String(attributed[run.range].characters)
                guard !text.isEmpty else { continue }
                var attributes = baseAttributes(for: role)
                let inlineIntent = run.inlinePresentationIntent
                attributes[.font] = font(for: role, inlineIntent: inlineIntent)
                attributes[.foregroundColor] = foregroundColor(for: role, inlineIntent: inlineIntent, link: run.link)

                if let link = run.link {
                    let scheme = link.scheme?.lowercased() ?? ""
                    if scheme == "https" || scheme == "http" {
                        attributes[.link] = link
                        attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue
                    }
                }

                if inlineIntent?.contains(.code) == true {
                    attributes[.backgroundColor] = NSColor.controlBackgroundColor
                }

                mutable.append(NSAttributedString(string: text, attributes: attributes))
            }

            if mutable.length == 0 {
                return NSAttributedString(string: markdown, attributes: baseAttributes(for: role))
            }

            return mutable
        }
    }

    private static func codeBlockString(_ code: String) -> NSAttributedString {
        NSAttributedString(
            string: code,
            attributes: baseAttributes(for: .codeBlock)
        )
    }

    private static func baseAttributes(for role: MarkdownTextRole) -> [NSAttributedString.Key: Any] {
        [
            .font: font(for: role, inlineIntent: nil),
            .foregroundColor: foregroundColor(for: role, inlineIntent: nil, link: nil),
            .paragraphStyle: paragraphStyle(for: role)
        ]
    }

    private static func paragraphStyle(for role: MarkdownTextRole) -> NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.lineBreakMode = .byWordWrapping

        switch role {
        case let .heading(level):
            style.lineSpacing = 3
            style.paragraphSpacing = level <= 2 ? 8 : 6
        case .body:
            style.lineSpacing = 4
            style.paragraphSpacing = 10
        case .listItem:
            style.lineSpacing = 4
            style.paragraphSpacing = 4
        case .blockQuote:
            style.lineSpacing = 4
            style.paragraphSpacing = 6
        case .codeBlock:
            style.lineSpacing = 2
            style.paragraphSpacing = 0
        case .tableHeader, .tableCell:
            style.lineSpacing = 2
            style.paragraphSpacing = 0
        }

        return style
    }

    private static func font(for role: MarkdownTextRole, inlineIntent: InlinePresentationIntent?) -> NSFont {
        let baseFont: NSFont
        switch role {
        case let .heading(level):
            switch level {
            case 1:
                baseFont = .systemFont(ofSize: 28, weight: .semibold)
            case 2:
                baseFont = .systemFont(ofSize: 24, weight: .semibold)
            case 3:
                baseFont = .systemFont(ofSize: 20, weight: .semibold)
            default:
                baseFont = .systemFont(ofSize: 17, weight: .semibold)
            }
        case .body, .listItem, .blockQuote, .tableCell:
            baseFont = .systemFont(ofSize: 14)
        case .tableHeader:
            baseFont = .systemFont(ofSize: 13, weight: .semibold)
        case .codeBlock:
            baseFont = .monospacedSystemFont(ofSize: 13, weight: .regular)
        }

        guard let inlineIntent else { return baseFont }
        if inlineIntent.contains(.code) {
            return .monospacedSystemFont(
                ofSize: max(baseFont.pointSize - 1, 12),
                weight: inlineIntent.contains(.stronglyEmphasized) ? .semibold : .regular
            )
        }

        var font = baseFont
        if inlineIntent.contains(.stronglyEmphasized) {
            font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
        }
        if inlineIntent.contains(.emphasized) {
            font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
        }
        return font
    }

    private static func foregroundColor(for role: MarkdownTextRole, inlineIntent: InlinePresentationIntent?, link: URL?) -> NSColor {
        if link != nil { return .linkColor }
        switch role {
        case .blockQuote:
            return .secondaryLabelColor
        case .tableHeader:
            return .labelColor
        case .codeBlock:
            return .labelColor
        case .heading, .body, .listItem, .tableCell:
            return .labelColor
        }
    }
}

private struct MarkdownRichTextBlockView: View {
    let content: String
    let role: MarkdownTextRole

    var body: some View {
        MarkdownDocumentTextView(
            attributedString: MarkdownAttributedStringBuilder.build(markdown: content, role: role),
            backgroundColor: .clear
        )
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct MarkdownListBlockView: View {
    let items: [MarkdownListItem]
    let ordered: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                HStack(alignment: .top, spacing: 12) {
                    Text(marker(for: item, index: index))
                        .font(.system(size: 13, weight: .semibold, design: .rounded))
                        .foregroundStyle(.secondary)
                        .frame(width: 28, alignment: .trailing)
                    MarkdownRichTextBlockView(content: item.text, role: .listItem)
                }
            }
        }
    }

    private func marker(for item: MarkdownListItem, index: Int) -> String {
        if ordered {
            return "\(item.ordinal ?? (index + 1))."
        }
        return "•"
    }
}

private struct MarkdownBlockQuoteView: View {
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            RoundedRectangle(cornerRadius: 999, style: .continuous)
                .fill(Color.secondary.opacity(0.35))
                .frame(width: 4)
            MarkdownRichTextBlockView(content: text, role: .blockQuote)
        }
        .padding(.vertical, 2)
    }
}

private struct MarkdownCodeBlockView: View {
    let language: String?
    let code: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let language, !language.isEmpty {
                Text(language.uppercased())
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
            }

            MarkdownDocumentTextView(
                attributedString: MarkdownAttributedStringBuilder.build(markdown: code, role: .codeBlock),
                backgroundColor: NSColor.controlBackgroundColor
            )
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(Color.secondary.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
    }
}

private struct MarkdownTableView: View {
    let table: MarkdownTableBlock

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Grid(alignment: .leading, horizontalSpacing: 0, verticalSpacing: 0) {
                GridRow {
                    ForEach(Array(table.header.enumerated()), id: \.offset) { _, cell in
                        tableCell(content: cell, role: .tableHeader, fill: Color.secondary.opacity(0.08))
                    }
                }

                ForEach(Array(table.rows.enumerated()), id: \.offset) { rowIndex, row in
                    GridRow {
                        ForEach(Array(row.enumerated()), id: \.offset) { _, cell in
                            tableCell(
                                content: cell,
                                role: .tableCell,
                                fill: rowIndex.isMultiple(of: 2) ? Color.clear : Color.secondary.opacity(0.03)
                            )
                        }
                    }
                }
            }
        }
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(Color.secondary.opacity(0.15), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func tableCell(content: String, role: MarkdownTextRole, fill: Color) -> some View {
        MarkdownDocumentTextView(
            attributedString: MarkdownAttributedStringBuilder.build(markdown: content, role: role),
            backgroundColor: .clear
        )
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(fill)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.secondary.opacity(0.12))
                .frame(height: 1)
        }
    }
}

private struct MarkdownDocumentTextView: NSViewRepresentable {
    let attributedString: NSAttributedString
    let backgroundColor: NSColor

    func makeNSView(context: Context) -> MarkdownIntrinsicTextView {
        let textView = MarkdownIntrinsicTextView()
        textView.drawsBackground = backgroundColor.alphaComponent > 0.001
        textView.backgroundColor = backgroundColor
        textView.isEditable = false
        textView.isSelectable = true
        textView.isRichText = true
        textView.importsGraphics = false
        textView.usesFindBar = false
        textView.textContainerInset = NSSize(width: 0, height: 0)
        textView.textContainer?.lineFragmentPadding = 0
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.containerSize = NSSize(width: 1, height: CGFloat.greatestFiniteMagnitude)
        textView.isHorizontallyResizable = false
        textView.isVerticallyResizable = true
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.linkTextAttributes = [
            .foregroundColor: NSColor.linkColor,
            .underlineStyle: NSUnderlineStyle.single.rawValue
        ]
        textView.applyAttributedStringIfNeeded(attributedString)
        return textView
    }

    func updateNSView(_ nsView: MarkdownIntrinsicTextView, context: Context) {
        nsView.drawsBackground = backgroundColor.alphaComponent > 0.001
        nsView.backgroundColor = backgroundColor
        if nsView.applyAttributedStringIfNeeded(attributedString) {
            nsView.scheduleIntrinsicSizeInvalidation()
        }
    }
}

enum MarkdownTextViewUpdatePolicy {
    static func needsAttributedStringUpdate(current: NSAttributedString?, incoming: NSAttributedString) -> Bool {
        guard let current else { return true }
        return current.isEqual(to: incoming) == false
    }

    static func shouldInvalidateLayout(
        previousWidth: CGFloat?,
        newWidth: CGFloat,
        tolerance: CGFloat = 0.5
    ) -> Bool {
        guard let previousWidth else { return true }
        return abs(previousWidth - newWidth) > tolerance
    }
}

private final class MarkdownIntrinsicTextView: NSTextView {
    private var lastMeasuredWidth: CGFloat?
    private var intrinsicInvalidationScheduled = false

    override var intrinsicContentSize: NSSize {
        guard let textContainer, let layoutManager else {
            return NSSize(width: NSView.noIntrinsicMetric, height: 0)
        }
        layoutManager.ensureLayout(for: textContainer)
        let usedRect = layoutManager.usedRect(for: textContainer)
        return NSSize(
            width: NSView.noIntrinsicMetric,
            height: ceil(usedRect.height + textContainerInset.height * 2)
        )
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        textContainer?.containerSize = NSSize(width: max(newSize.width, 1), height: CGFloat.greatestFiniteMagnitude)
        if MarkdownTextViewUpdatePolicy.shouldInvalidateLayout(previousWidth: lastMeasuredWidth, newWidth: newSize.width) {
            lastMeasuredWidth = newSize.width
            scheduleIntrinsicSizeInvalidation()
        }
    }

    @discardableResult
    func applyAttributedStringIfNeeded(_ attributedString: NSAttributedString) -> Bool {
        let current = textStorage?.copy() as? NSAttributedString
        guard MarkdownTextViewUpdatePolicy.needsAttributedStringUpdate(current: current, incoming: attributedString) else {
            return false
        }
        textStorage?.setAttributedString(attributedString)
        return true
    }

    func scheduleIntrinsicSizeInvalidation() {
        guard intrinsicInvalidationScheduled == false else { return }
        intrinsicInvalidationScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.intrinsicInvalidationScheduled = false
            self.invalidateIntrinsicContentSize()
        }
    }
}

private struct MarkdownImageBlockView: View {
    let block: MarkdownImageBlock

    var body: some View {
        if let resolvedURL = block.resolvedURL,
           let image = NSImage(contentsOf: resolvedURL) {
            VStack(alignment: .leading, spacing: 6) {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFit()
                    .frame(maxHeight: 320)
                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                if !block.altText.isEmpty {
                    Text(block.altText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        } else {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: block.isAllowed ? "photo" : "photo.badge.exclamationmark")
                    .foregroundStyle(block.isAllowed ? AnyShapeStyle(.secondary) : AnyShapeStyle(Color.orange))
                VStack(alignment: .leading, spacing: 4) {
                    Text(block.altText.isEmpty ? "Image" : block.altText)
                        .font(.caption.bold())
                    Text(block.source)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.secondary.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
    }
}

struct JSONTreeDocumentView: View {
    let rawJSON: String
    @State private var expandedPaths: Set<String>
    private let parsedRoot: JSONTreeNode?

    init(rawJSON: String) {
        self.rawJSON = rawJSON
        let parsedRoot = try? JSONTreeNode.parse(rawJSON)
        self.parsedRoot = parsedRoot
        let seedDepth = rawJSON.count <= 4_096 ? 1 : 0
        self._expandedPaths = State(initialValue: parsedRoot?.expansionSeed(maxDepth: seedDepth) ?? [])
    }

    var body: some View {
        if let parsedRoot {
            JSONTreeNodeView(
                node: parsedRoot,
                key: nil,
                depth: 0,
                expandedPaths: $expandedPaths
            )
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            VStack(alignment: .leading, spacing: 8) {
                Label("JSON parse failed", systemImage: "exclamationmark.triangle")
                    .font(.caption)
                    .foregroundStyle(Color.orange)
                PlainTextArtifactView(content: rawJSON, monospaced: true)
            }
        }
    }
}

private struct ProposalReviewSummaryArtifactView: View {
    let rawJSON: String
    private let presentation: ProposalReviewSummaryPresentation?

    init(rawJSON: String) {
        self.rawJSON = rawJSON
        self.presentation = ProposalReviewSummaryPresentation.parse(rawJSON)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            if let presentation {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 10) {
                        Text(presentation.pass ? "Approved" : "Blocked")
                            .font(.headline)
                        Text("Blockers: \(presentation.blockerCount)")
                            .foregroundStyle(.secondary)
                    }

                    if let summary = presentation.summary {
                        Text(summary)
                            .foregroundStyle(.primary)
                    }

                    proposalSection("Blocking Issues", items: presentation.blockingIssues)
                    proposalSection(
                        "Blocking Required Changes",
                        items: presentation.blockingRequiredChanges
                    )
                    proposalSection("Advisory Follow-Ups", items: presentation.advisoryFollowUps)
                    proposalSection("Recurring Themes", items: presentation.recurringThemes)
                }
                .padding(12)
                .background(Color.secondary.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            }

            DisclosureGroup("Raw JSON") {
                JSONTreeDocumentView(rawJSON: rawJSON)
                    .padding(.top, 8)
            }
        }
    }

    @ViewBuilder
    private func proposalSection(_ title: String, items: [String]) -> some View {
        if items.isEmpty == false {
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                ForEach(items, id: \.self) { item in
                    Text("• \(item)")
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                }
            }
        }
    }
}

struct ProposalReviewSummaryPresentation: Equatable {
    let pass: Bool
    let blockerCount: Int
    let summary: String?
    let blockingIssues: [String]
    let blockingRequiredChanges: [String]
    let advisoryFollowUps: [String]
    let recurringThemes: [String]

    static func parse(_ rawJSON: String) -> ProposalReviewSummaryPresentation? {
        guard let data = rawJSON.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let pass = object["pass"] as? Bool,
              let blockerCount = object["blocker_count"] as? Int
        else {
            return nil
        }

        return ProposalReviewSummaryPresentation(
            pass: pass,
            blockerCount: blockerCount,
            summary: object["summary"] as? String,
            blockingIssues: renderJSONArray(object["blocking_issues"]),
            blockingRequiredChanges: renderJSONArray(
                object["blocking_required_changes"] ?? object["required_changes"]
            ),
            advisoryFollowUps: renderJSONArray(object["advisory_follow_ups"]),
            recurringThemes: renderJSONArray(object["recurring_themes"])
        )
    }

    nonisolated private static func renderJSONArray(_ value: Any?) -> [String] {
        guard let values = value as? [Any] else { return [] }
        return values.compactMap(renderJSONValue)
    }

    nonisolated private static func renderJSONValue(_ value: Any) -> String? {
        if let string = value as? String {
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        }
        if let dict = value as? [String: Any] {
            if let id = dict["id"] as? String, id.isEmpty == false,
               let summary = dict["summary"] as? String, summary.isEmpty == false {
                return "\(id): \(summary)"
            }
            if let id = dict["id"] as? String, id.isEmpty == false {
                return id
            }
            if let data = try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys]),
               let string = String(data: data, encoding: .utf8) {
                return string
            }
        }
        return String(describing: value)
    }
}

private struct JSONTreeNodeView: View {
    let node: JSONTreeNode
    let key: String?
    let depth: Int
    @Binding var expandedPaths: Set<String>

    var body: some View {
        switch node.kind {
        case let .object(entries):
            DisclosureGroup(isExpanded: expansionBinding) {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(entries.enumerated()), id: \.offset) { _, entry in
                        JSONTreeNodeView(
                            node: entry.node,
                            key: entry.key,
                            depth: depth + 1,
                            expandedPaths: $expandedPaths
                        )
                    }
                }
                .padding(.top, 6)
            } label: {
                JSONTreeLabel(key: key, valueSummary: node.collapsedSummary, valueColor: .secondary)
            }
            .padding(.leading, CGFloat(depth) * 14)

        case let .array(elements):
            DisclosureGroup(isExpanded: expansionBinding) {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(elements.enumerated()), id: \.offset) { index, element in
                        JSONTreeNodeView(
                            node: element,
                            key: "[\(index)]",
                            depth: depth + 1,
                            expandedPaths: $expandedPaths
                        )
                    }
                }
                .padding(.top, 6)
            } label: {
                JSONTreeLabel(key: key, valueSummary: node.collapsedSummary, valueColor: .secondary)
            }
            .padding(.leading, CGFloat(depth) * 14)

        case let .string(value):
            JSONTreeLabel(key: key, valueSummary: "\"\(value)\"", valueColor: .green)
                .padding(.leading, CGFloat(depth) * 14)

        case let .number(value):
            JSONTreeLabel(key: key, valueSummary: value, valueColor: .blue)
                .padding(.leading, CGFloat(depth) * 14)

        case let .boolean(value):
            JSONTreeLabel(key: key, valueSummary: value ? "true" : "false", valueColor: .orange)
                .padding(.leading, CGFloat(depth) * 14)

        case .null:
            JSONTreeLabel(key: key, valueSummary: "null", valueColor: .secondary)
                .padding(.leading, CGFloat(depth) * 14)
        }
    }

    private var expansionBinding: Binding<Bool> {
        Binding(
            get: { expandedPaths.contains(node.path) },
            set: { isExpanded in
                if isExpanded {
                    expandedPaths.insert(node.path)
                } else {
                    expandedPaths.remove(node.path)
                }
            }
        )
    }
}

private struct JSONTreeLabel: View {
    let key: String?
    let valueSummary: String
    let valueColor: Color

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            if let key {
                Text(key)
                    .font(.system(.body, design: .monospaced))
                    .foregroundStyle(.primary)
            }
            Text(valueSummary)
                .font(.system(.body, design: .monospaced))
                .foregroundStyle(valueColor)
            Spacer(minLength: 0)
        }
        .textSelection(.enabled)
    }
}

struct PlainTextArtifactView: View {
    let content: String
    let monospaced: Bool

    var body: some View {
        Text(content)
            .frame(maxWidth: .infinity, alignment: .leading)
            .textSelection(.enabled)
            .font(monospaced ? .body.monospaced() : .body)
    }
}

struct DiffArtifactView: View {
    let content: String

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            ForEach(Array(content.components(separatedBy: .newlines).enumerated()), id: \.offset) { _, line in
                Text(line)
                    .font(.body.monospaced())
                    .foregroundStyle(diffLineColor(line))
                    .background(diffLineBackground(line))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .textSelection(.enabled)
    }

    private func diffLineColor(_ line: String) -> Color {
        if line.hasPrefix("+") { return .green }
        if line.hasPrefix("-") { return .red }
        if line.hasPrefix("@@") { return .blue }
        return .primary
    }

    private func diffLineBackground(_ line: String) -> Color {
        if line.hasPrefix("+") { return .green.opacity(0.1) }
        if line.hasPrefix("-") { return .red.opacity(0.1) }
        return .clear
    }
}

struct WorkflowRunArtifactSnapshot {
    let latestArtifacts: [Artifact]
    let approvalContextArtifacts: [Artifact]
    let latestDebugArtifacts: [Artifact]

    init(artifacts: [Artifact]) {
        let visibleArtifacts = artifacts.filter { $0.reportKind != "immutable_history" }
        self.latestArtifacts = visibleArtifacts.sorted { lhs, rhs in
            if lhs.name == "final_feature_report" && rhs.name != "final_feature_report" {
                return true
            }
            if rhs.name == "final_feature_report" && lhs.name != "final_feature_report" {
                return false
            }
            return lhs.createdAt > rhs.createdAt
        }

        self.approvalContextArtifacts = visibleArtifacts
            .filter { artifact in
                artifact.name == "proposal_review_summary" || artifact.name == "proposal_current"
            }
            .reduce(into: [String: Artifact]()) { latestByName, artifact in
                if let current = latestByName[artifact.name], current.createdAt >= artifact.createdAt {
                    return
                }
                latestByName[artifact.name] = artifact
            }
            .values
            .sorted { lhs, rhs in
                Self.approvalContextRank(lhs.name) < Self.approvalContextRank(rhs.name)
            }

        self.latestDebugArtifacts = visibleArtifacts
            .filter { artifact in
                artifact.name.contains("transcript") || artifact.name.contains("receipt")
            }
            .sorted { $0.createdAt > $1.createdAt }
    }

    private static func approvalContextRank(_ name: String) -> Int {
        switch name {
        case "proposal_review_summary":
            return 0
        case "proposal_current":
            return 1
        default:
            return 2
        }
    }
}

enum ArtifactInspectorSkillTruthFormatter {
    static func compactSummary(_ summary: String) -> String? {
        let compacted = summary
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        guard !compacted.isEmpty else { return nil }
        let maxLength = 221
        guard compacted.count > maxLength else { return compacted }
        return String(compacted.prefix(maxLength - 1)) + "…"
    }
}

enum ArtifactInspectorTraceabilityResolver {
    static func downstreamConsumers(
        artifact: Artifact,
        run: Run,
        modelContext: ModelContext
    ) -> [AgentExecution] {
        let runID = run.id
        let descriptor = FetchDescriptor<StageExecution>()
        guard let stages = try? modelContext.fetch(descriptor) else { return [] }

        return stages
            .filter { $0.run?.id == runID }
            .flatMap { stage in stage.agentExecutions }
            .filter { execution in
                guard let data = execution.inputBindingsJSON,
                      let bindings = try? JSONDecoder().decode([InputBinding].self, from: data)
                else { return false }
                return bindings.contains { binding in
                    binding.artifactName == artifact.name && binding.producingAgentID == artifact.agentID
                }
            }
    }
}

enum RunReportSupersedence {
    static func notice(for artifact: Artifact, run: Run) -> String? {
        guard artifact.reportKind == "immutable_history",
              let reportVersion = artifact.reportVersion,
              reportVersion < run.latestReportVersion
        else { return nil }

        return "This immutable run report was superseded after the run continued to version \(run.latestReportVersion)."
    }
}
