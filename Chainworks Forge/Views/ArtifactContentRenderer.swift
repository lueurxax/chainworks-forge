import SwiftUI
import Foundation
import AppKit

enum ArtifactRenderProvenance: Equatable {
    case artifactBacked
    case explicit
}

struct ArtifactRenderContext: Equatable {
    let format: ArtifactFormat
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
            localRoots: deduplicatedRoots(roots),
            provenance: .artifactBacked
        )
    }

    static func explicit(format: ArtifactFormat, localRoots: [URL] = []) -> ArtifactRenderContext {
        ArtifactRenderContext(
            format: format,
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

enum MarkdownImageSourcePolicy {
    case v1

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

        var allowedCandidates: [URL] = []
        for root in localRoots {
            let candidate = root.appendingPathComponent(trimmed).standardizedFileURL
            if isAllowed(candidate, within: localRoots) {
                if FileManager.default.fileExists(atPath: candidate.path) {
                    return candidate
                }
                allowedCandidates.append(candidate)
            }
        }

        return allowedCandidates.first
    }

    private func resolveLocalURL(_ url: URL, localRoots: [URL]) -> URL? {
        let standardized = url.standardizedFileURL
        return isAllowed(standardized, within: localRoots) ? standardized : nil
    }

    private func isAllowed(_ url: URL, within roots: [URL]) -> Bool {
        let path = url.standardizedFileURL.path
        return roots.contains { root in
            let rootPath = root.standardizedFileURL.path
            return path == rootPath || path.hasPrefix(rootPath + "/")
        }
    }
}

enum MarkdownDocumentBlockKind: Equatable {
    case markdown
    case image
}

struct MarkdownImageBlock: Equatable {
    let altText: String
    let source: String
    let resolvedURL: URL?
    let isAllowed: Bool
}

enum MarkdownDocumentBlock: Equatable {
    case markdown(String)
    case image(MarkdownImageBlock)

    var kind: MarkdownDocumentBlockKind {
        switch self {
        case .markdown: return .markdown
        case .image: return .image
        }
    }
}

enum MarkdownDocumentParser {
    private static let imagePattern = try! NSRegularExpression(
        pattern: #"^\s*!\[([^\]]*)\]\(([^)]+)\)\s*$"#,
        options: []
    )

    static func parse(_ content: String, localRoots: [URL], policy: MarkdownImageSourcePolicy = .v1) -> [MarkdownDocumentBlock] {
        let normalized = content.replacingOccurrences(of: "\r\n", with: "\n")
        let paragraphs = normalized.components(separatedBy: "\n\n")
        var blocks: [MarkdownDocumentBlock] = []

        for paragraph in paragraphs {
            let trimmed = paragraph.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { continue }

            if let imageBlock = parseStandaloneImage(trimmed, localRoots: localRoots, policy: policy) {
                blocks.append(.image(imageBlock))
            } else {
                blocks.append(.markdown(trimmed))
            }
        }

        if blocks.isEmpty, !content.isEmpty {
            blocks.append(.markdown(content))
        }

        return mergeMarkdownBlocks(blocks)
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

    private static func mergeMarkdownBlocks(_ blocks: [MarkdownDocumentBlock]) -> [MarkdownDocumentBlock] {
        var merged: [MarkdownDocumentBlock] = []

        for block in blocks {
            switch block {
            case let .markdown(text):
                if case let .markdown(existing)? = merged.last {
                    merged.removeLast()
                    merged.append(.markdown(existing + "\n\n" + text))
                } else {
                    merged.append(.markdown(text))
                }
            case .image:
                merged.append(block)
            }
        }

        return merged
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

    var body: some View {
        switch context.format {
        case .markdown:
            MarkdownDocumentView(content: content, localRoots: context.localRoots)
        case .json:
            JSONTreeDocumentView(rawJSON: content)
        case .diff:
            DiffArtifactView(content: content)
        case .report:
            PlainTextArtifactView(content: content, monospaced: true)
        }
    }
}

struct MarkdownDocumentView: View {
    let content: String
    let localRoots: [URL]

    private var blocks: [MarkdownDocumentBlock] {
        MarkdownDocumentParser.parse(content, localRoots: localRoots)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                switch block {
                case let .markdown(text):
                    MarkdownTextBlockView(content: text)
                case let .image(imageBlock):
                    MarkdownImageBlockView(block: imageBlock)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct MarkdownTextBlockView: View {
    let content: String

    var body: some View {
        let attributed = (try? AttributedString(
            markdown: content,
            options: .init(interpretedSyntax: .full)
        )) ?? AttributedString(content)

        Text(attributed)
            .frame(maxWidth: .infinity, alignment: .leading)
            .textSelection(.enabled)
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
