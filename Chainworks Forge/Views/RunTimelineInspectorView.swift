import SwiftUI

struct RunTimelineInspectorView: View {
    let projection: WorkflowMapProjection
    var showsTitle: Bool = true

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if showsTitle {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Live Timeline")
                                .font(.title3.weight(.semibold))
                            Text("Live stream of agent events during the current run.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    if projection.liveTimeline.isEmpty {
                        ContentUnavailableView(
                            "No Timeline Data",
                            systemImage: "waveform.path.ecg",
                            description: Text("No live events yet.")
                        )
                        .frame(maxWidth: .infinity)
                        .frame(maxHeight: .infinity)
                    } else {
                        GroupBox("Live Stream") {
                            VStack(alignment: .leading, spacing: 10) {
                                ForEach(projection.liveTimeline) { entry in
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(entry.agentTitle)
                                                .font(.subheadline.weight(.semibold))
                                            Spacer()
                                            Text(entry.event.type.rawValue)
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }

                                        TimelineEventDetailView(event: entry.event)

                                        HStack(spacing: 8) {
                                            Text(entry.stageID)
                                            if let sessionID = entry.event.sessionID {
                                                Text(sessionID)
                                            }
                                            Text(entry.event.timestamp, format: .dateTime.hour().minute().second())
                                        }
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(10)
                                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                                    .transition(.asymmetric(
                                        insertion: .push(from: .bottom).combined(with: .opacity),
                                        removal: .opacity
                                    ))
                                }
                            }
                        }
                        .animation(.spring(response: 0.45, dampingFraction: 0.82), value: projection.liveTimeline.map(\.id))
                    }

                    // Невидимый якорь для автопрокрутки к последней записи
                    Color.clear
                        .frame(height: 1)
                        .id("live-timeline-bottom")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            }
            .onChange(of: projection.liveTimeline.count) {
                withAnimation(.spring(response: 0.45, dampingFraction: 0.82)) {
                    proxy.scrollTo("live-timeline-bottom", anchor: .bottom)
                }
            }
        }
        .frame(minWidth: 480, minHeight: 420)
        .accessibilityIdentifier("run-timeline-inspector-view")
    }
}

private struct TimelineEventDetailView: View {
    let event: ExecutionEvent

    var body: some View {
        switch event.type {
        case .textChunk:
            StreamingTimelineTextView(text: event.detail)
        case .error:
            TimelineErrorDetailView(presentation: TimelineErrorPresentation(rawDetail: event.detail))
        default:
            Text(event.detail)
                .font(.caption)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
        }
    }
}

struct TimelineErrorPresentation: Equatable {
    let summary: String
    let highlights: [String]
    let rawDetail: String

    init(rawDetail: String) {
        let lines = rawDetail
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        if rawDetail.localizedCaseInsensitiveContains("JavaScript heap out of memory") {
            summary = "Gemini CLI failed: JavaScript heap out of memory."
        } else if let first = lines.first {
            summary = TimelineErrorPresentation.compactLine(
                first.replacingOccurrences(of: "Request failed: ", with: "")
            )
        } else {
            summary = "Execution failed."
        }

        let markers = [
            "out of memory",
            "operation not permitted",
            "could not connect",
            "rate limit",
            "authentication",
            "eperm",
            "eacces",
            "enoent",
            "exit code"
        ]

        let resolvedSummary = summary
        var seen = Set<String>()
        highlights = lines
            .filter { line in
                let lowercased = line.lowercased()
                return markers.contains(where: lowercased.contains)
            }
            .map { TimelineErrorPresentation.compactLine($0) }
            .filter { seen.insert($0).inserted && $0 != resolvedSummary }
            .prefix(4)
            .map { $0 }

        self.rawDetail = rawDetail
    }

    var shouldOfferRawDisclosure: Bool {
        let normalizedRaw = rawDetail.trimmingCharacters(in: .whitespacesAndNewlines)
        return !normalizedRaw.isEmpty && normalizedRaw != summary
    }

    nonisolated private static func compactLine(_ line: String) -> String {
        let collapsed = line.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        if collapsed.count > 220 {
            return String(collapsed.prefix(217)) + "..."
        }
        return collapsed
    }
}

private struct TimelineErrorDetailView: View {
    let presentation: TimelineErrorPresentation
    @State private var showRawDetail = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Request failed", systemImage: "exclamationmark.triangle.fill")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.red)

            Text(presentation.summary)
                .font(.caption)
                .foregroundStyle(.primary)
                .textSelection(.enabled)

            if !presentation.highlights.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(presentation.highlights, id: \.self) { highlight in
                        HStack(alignment: .top, spacing: 6) {
                            Image(systemName: "circle.fill")
                                .font(.system(size: 4))
                                .foregroundStyle(.secondary)
                                .padding(.top, 6)
                            Text(highlight)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                }
            }

            if presentation.shouldOfferRawDisclosure {
                DisclosureGroup(showRawDetail ? "Hide raw error" : "Show raw error", isExpanded: $showRawDetail) {
                    ScrollView {
                        Text(presentation.rawDetail)
                            .font(.caption2.monospaced())
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .frame(maxHeight: 220)
                    .padding(.top, 4)
                }
                .font(.caption2)
            }
        }
    }
}

private struct StreamingTimelineTextView: View {
    let text: String
    @State private var displayedText: String = ""
    private var presentation: StreamingTimelineTextPresentation {
        StreamingTimelineTextPresentation(rawText: displayedText)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label("Streaming output", systemImage: "text.line.first.and.arrowtriangle.forward")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)

            switch presentation.kind {
            case .plainText:
                Text(displayedText)
                    .font(.caption)
                    .foregroundStyle(.primary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            case .providerError:
                StreamingTimelineProviderErrorView(presentation: presentation)
            }
        }
        .onAppear {
            displayedText = text
        }
        .onChange(of: text) {
            displayedText = text
        }
    }
}

struct StreamingTimelineTextPresentation: Equatable {
    enum Kind: Equatable {
        case plainText
        case providerError
    }

    let kind: Kind
    let summary: String
    let highlights: [String]
    let rawText: String

    init(rawText: String) {
        let trimmed = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        let lines = trimmed
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        if Self.looksLikeProviderRuntimeStack(lines: lines, rawText: trimmed) {
            kind = .providerError
            summary = Self.providerErrorSummary(lines: lines, rawText: trimmed)
            highlights = Self.providerErrorHighlights(lines: lines, summary: summary)
        } else {
            kind = .plainText
            summary = trimmed
            highlights = []
        }
        self.rawText = rawText
    }

    var shouldOfferRawDisclosure: Bool {
        kind == .providerError && rawText.trimmingCharacters(in: .whitespacesAndNewlines) != summary
    }

    private static func looksLikeProviderRuntimeStack(lines: [String], rawText: String) -> Bool {
        guard lines.isEmpty == false else { return false }
        let lowercased = rawText.lowercased()
        let hasRetryBoilerplate = lowercased.contains("please retry if you think this is a transient or recoverable error")
        let hasNodePath = lowercased.contains("/usr/local/bin/node") || lowercased.contains("/opt/homebrew/bin/node")
        let stackLineCount = lines.filter { line in
            line.range(of: #"^\d+:\s+0x[0-9a-f]+.*"#, options: .regularExpression) != nil
        }.count
        let hasFatalMarker = lowercased.contains("fatal error") || lowercased.contains("javascript heap out of memory")
        let hasRequestFailedPrefix = lowercased.contains("request failed:")
        return (hasNodePath && stackLineCount >= 3) || hasFatalMarker || (hasRetryBoilerplate && hasRequestFailedPrefix)
    }

    private static func providerErrorSummary(lines: [String], rawText: String) -> String {
        let lowercased = rawText.lowercased()
        if lowercased.contains("javascript heap out of memory") {
            return "Provider runtime emitted a Node.js out-of-memory failure."
        }
        if let requestLine = lines.first(where: { $0.lowercased().contains("request failed:") }) {
            let compact = compactLine(requestLine.replacingOccurrences(of: "Request failed: ", with: ""))
            return "Provider runtime emitted an internal error: \(compact)"
        }
        return "Provider runtime emitted an internal Node.js error."
    }

    private static func providerErrorHighlights(lines: [String], summary: String) -> [String] {
        let interesting = lines.filter { line in
            let lowercased = line.lowercased()
            return lowercased.contains("request failed")
                || lowercased.contains("fatal error")
                || lowercased.contains("out of memory")
                || lowercased.contains("/usr/local/bin/node")
                || lowercased.contains("/opt/homebrew/bin/node")
        }

        var seen = Set<String>()
        return interesting
            .map(compactLine)
            .filter { seen.insert($0).inserted && $0 != summary }
            .prefix(4)
            .map { $0 }
    }

    private static func compactLine(_ line: String) -> String {
        let collapsed = line.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        if collapsed.count > 220 {
            return String(collapsed.prefix(217)) + "..."
        }
        return collapsed
    }
}

private struct StreamingTimelineProviderErrorView: View {
    let presentation: StreamingTimelineTextPresentation
    @State private var showRawText = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Provider runtime error output", systemImage: "exclamationmark.triangle.fill")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.orange)

            Text(presentation.summary)
                .font(.caption)
                .foregroundStyle(.primary)
                .textSelection(.enabled)

            if !presentation.highlights.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(presentation.highlights, id: \.self) { highlight in
                        HStack(alignment: .top, spacing: 6) {
                            Image(systemName: "circle.fill")
                                .font(.system(size: 4))
                                .foregroundStyle(.secondary)
                                .padding(.top, 6)
                            Text(highlight)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                }
            }

            if presentation.shouldOfferRawDisclosure {
                DisclosureGroup(showRawText ? "Hide raw output" : "Show raw output", isExpanded: $showRawText) {
                    ScrollView {
                        Text(presentation.rawText)
                            .font(.caption2.monospaced())
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .frame(maxHeight: 220)
                    .padding(.top, 4)
                }
                .font(.caption2)
            }
        }
    }
}
