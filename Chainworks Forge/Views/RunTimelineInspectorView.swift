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

    private static func compactLine(_ line: String) -> String {
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

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label("Streaming output", systemImage: "text.line.first.and.arrowtriangle.forward")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)

            ZStack(alignment: .leading) {
                Text(displayedText)
                    .id(displayedText)
                    .font(.caption)
                    .foregroundStyle(.primary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
                    .transition(.opacity.combined(with: .move(edge: .bottom)))
            }
            .animation(.easeOut(duration: 0.18), value: displayedText)
        }
        .onAppear {
            displayedText = text
        }
        .onChange(of: text) {
            displayedText = text
        }
    }
}
