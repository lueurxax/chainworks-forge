import Combine
import SwiftUI

@MainActor
final class P036TimelinePresentationModel: ObservableObject {
    @Published private(set) var entries: [FocusedTimelineSpineEntry] = []

    private var buffer: (live: [LiveExecutionTimelineEntry], persisted: [WorkflowMapPersistedTimelineEntry], xcode: [WorkflowMapXcodeRuntimeObservation])?

    func update(
        live: [LiveExecutionTimelineEntry],
        persisted: [WorkflowMapPersistedTimelineEntry],
        xcode: [WorkflowMapXcodeRuntimeObservation],
        implementationCompletion: P088ImplementationCompletionPresentation? = nil,
        generatedAt: Date = Date()
    ) {
        buffer = (live, persisted, xcode)
        entries = buildFocusedTimelineSpineEntries(
            liveTimeline: live,
            persistedTimeline: persisted,
            xcodeRuntimeObservations: xcode,
            implementationCompletion: implementationCompletion,
            implementationCompletionTimestamp: generatedAt
        )
    }
}

struct FocusedTimelineSpineEntry: Identifiable, Sendable {
    let id: String
    let kind: EntryKind
    let title: String
    let detail: String
    let timestamp: Date
    let stageID: String
    let surfaceLabel: String
    let sessionID: String?
    let agentID: String?
    let isCollapsed: Bool
    let liveEvent: ExecutionEvent?

    enum EntryKind: String, Sendable {
        case text
        case mergedTool = "merged_tool"
        case sessionEvent = "session_event"
        case agentSummary = "agent_summary"
        case policyWarning = "policy_warning"
        case implementationCompletion = "implementation_completion"
        case persisted
    }
}

func buildFocusedTimelineSpineEntries(
    liveTimeline: [LiveExecutionTimelineEntry],
    persistedTimeline: [WorkflowMapPersistedTimelineEntry],
    xcodeRuntimeObservations: [WorkflowMapXcodeRuntimeObservation] = [],
    implementationCompletion: P088ImplementationCompletionPresentation? = nil,
    implementationCompletionTimestamp: Date = Date()
) -> [FocusedTimelineSpineEntry] {
    var rawEntries: [FocusedTimelineSpineEntry] = []

    // Group live events by requestID for reconciliation
    var toolCalls: [String: (start: LiveExecutionTimelineEntry, finish: LiveExecutionTimelineEntry?)] = [:]
    var otherLive: [LiveExecutionTimelineEntry] = []

    for entry in liveTimeline {
        if let requestId = entry.event.requestID {
            if entry.event.type == .toolCallStarted {
                toolCalls[requestId] = (start: entry, finish: nil)
            } else if entry.event.type == .toolCallFinished {
                if var existing = toolCalls[requestId] {
                    existing.finish = entry
                    toolCalls[requestId] = existing
                } else {
                    otherLive.append(entry)
                }
            } else {
                otherLive.append(entry)
            }
        } else {
            otherLive.append(entry)
        }
    }

    // Create merged tool entries
    for (requestId, call) in toolCalls {
        let isFinished = call.finish != nil
        rawEntries.append(FocusedTimelineSpineEntry(
            id: requestId,
            kind: .mergedTool,
            title: call.start.agentTitle,
            detail: "Tool: \(call.start.event.toolName ?? "unknown") (\(isFinished ? "completed" : "running"))",
            timestamp: call.start.event.timestamp,
            stageID: call.start.stageID,
            surfaceLabel: "tool",
            sessionID: call.start.event.sessionID,
            agentID: nil,
            isCollapsed: false,
            liveEvent: call.start.event
        ))
    }

    // Map other live entries
    for entry in otherLive {
        let kind: FocusedTimelineSpineEntry.EntryKind = {
            switch entry.event.type {
            case .textChunk: return .text
            case .sessionStarted, .sessionClosed: return .sessionEvent
            case .finalOutput, .finish: return .agentSummary
            default: return .text
            }
        }()

        rawEntries.append(FocusedTimelineSpineEntry(
            id: entry.id.uuidString,
            kind: kind,
            title: entry.agentTitle,
            detail: entry.event.detail,
            timestamp: entry.event.timestamp,
            stageID: entry.stageID,
            surfaceLabel: entry.event.type.rawValue,
            sessionID: entry.event.sessionID,
            agentID: nil,
            isCollapsed: false,
            liveEvent: entry.event
        ))
    }

    // Map persisted entries
    for entry in persistedTimeline {
        rawEntries.append(FocusedTimelineSpineEntry(
            id: entry.id,
            kind: .persisted,
            title: entry.title,
            detail: entry.detail,
            timestamp: entry.timestamp,
            stageID: "persisted",
            surfaceLabel: "persisted",
            sessionID: entry.sessionID,
            agentID: nil,
            isCollapsed: false,
            liveEvent: nil
        ))
    }

    // Map policy warnings
    for observation in xcodeRuntimeObservations {
        for (index, warning) in observation.coalescedShimWarnings.enumerated() {
            rawEntries.append(FocusedTimelineSpineEntry(
                id: "\(observation.id)::policy-warning::\(index)",
                kind: .policyWarning,
                title: "Policy Warning",
                detail: "\(warning.policyReason): \(warning.matchedSubstring)",
                timestamp: warning.timestamp ?? Date(timeIntervalSince1970: 0),
                stageID: observation.stageID,
                surfaceLabel: "policy_warning",
                sessionID: nil,
                agentID: nil,
                isCollapsed: false,
                liveEvent: nil
            ))
        }
    }

    if let implementationCompletion {
        let detail = [
            implementationCompletion.outputFreshnessLabel,
            implementationCompletion.failureClassLabel,
            implementationCompletion.workChangeKindLabel,
            implementationCompletion.evidencePathLabel,
            implementationCompletion.nextOperatorActionLabel,
        ].compactMap { $0 }.joined(separator: ". ")
        rawEntries.append(FocusedTimelineSpineEntry(
            id: "implementation-completion::\(implementationCompletion.statusLabel)",
            kind: .implementationCompletion,
            title: implementationCompletion.compactSignalLabel,
            detail: detail,
            timestamp: implementationCompletionTimestamp,
            stageID: "run",
            surfaceLabel: "implementation_completion",
            sessionID: nil,
            agentID: "code_writer",
            isCollapsed: false,
            liveEvent: nil
        ))
    }

    // Sorting and simple collapse rules
    let sorted = rawEntries.sorted { lhs, rhs in
        if lhs.timestamp == rhs.timestamp {
            return lhs.id > rhs.id
        }
        return lhs.timestamp > rhs.timestamp
    }

    var finalEntries: [FocusedTimelineSpineEntry] = []
    for entry in sorted {
        if let last = finalEntries.last,
           last.kind == .text && entry.kind == .text,
           last.sessionID == entry.sessionID {
            // Collapse consecutive text from same session
            continue
        }
        finalEntries.append(entry)
    }

    return finalEntries
}

struct RunTimelineInspectorView: View {
    let projection: WorkflowMapProjection
    var showsTitle: Bool = true
    var implementationCompletion: P088ImplementationCompletionPresentation? = nil

    @StateObject private var model = P036TimelinePresentationModel()
    @Environment(\.accessibilityReduceMotion) var reduceMotion

    var body: some View {
        let bridgeProgressStatus = latestXcodeBridgeProgressStatus(
            in: projection.xcodeRuntimeObservations
        )
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if showsTitle {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Timeline")
                                .font(.title3.weight(.semibold))
                            Text("Focused run-detail timeline combining live execution and durable supervision history.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    if let bridgeProgressStatus {
                        Label {
                            Text(bridgeProgressStatus.label)
                                .font(.callout.weight(.semibold))
                        } icon: {
                            Image(systemName: bridgeProgressStatus.kind == .actionRequired ? "exclamationmark.shield" : "point.3.connected.trianglepath.dotted")
                        }
                        .foregroundStyle(bridgeProgressStatus.kind == .actionRequired ? .orange : .secondary)
                        .accessibilityIdentifier("xcode-bridge-progress-status")
                    }

                    if !projection.xcodeRuntimeObservations.isEmpty {
                        XcodeRuntimeObservationsView(observations: projection.xcodeRuntimeObservations)
                    }

                    if model.entries.isEmpty {
                        ContentUnavailableView(
                            "No Timeline Data",
                            systemImage: "waveform.path.ecg",
                            description: Text("No live or persisted timeline events yet.")
                        )
                        .frame(maxWidth: .infinity)
                        .frame(maxHeight: .infinity)
                    } else {
                        GroupBox("Timeline") {
                            VStack(alignment: .leading, spacing: 10) {
                                ForEach(model.entries) { entry in
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            entryIcon(for: entry.kind)
                                            Text(entry.title)
                                                .font(.subheadline.weight(.semibold))
                                            Spacer()
                                            Text(entry.surfaceLabel)
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }

                                        if entry.kind == .mergedTool {
                                            Label(entry.detail, systemImage: "hammer.fill")
                                                .font(.caption)
                                                .foregroundStyle(.primary)
                                                .padding(6)
                                                .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
                                        } else if let liveEvent = entry.liveEvent {
                                            TimelineEventDetailView(event: liveEvent)
                                        } else if entry.kind == .policyWarning {
                                            Label {
                                                Text(entry.detail)
                                                    .font(.caption)
                                                    .textSelection(.enabled)
                                            } icon: {
                                                Image(systemName: "exclamationmark.shield")
                                            }
                                            .foregroundStyle(.orange)
                                            .accessibilityIdentifier("xcode-policy-warning")
                                        } else {
                                            Text(entry.detail)
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
                                                .textSelection(.enabled)
                                        }

                                        HStack(spacing: 8) {
                                            Text(entry.stageID)
                                            if let sessionID = entry.sessionID {
                                                Text(sessionID)
                                            }
                                            Text(entry.timestamp, format: .dateTime.hour().minute().second())
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
                        .animation(reduceMotion ? .linear(duration: 0.1) : .spring(response: 0.45, dampingFraction: 0.82), value: model.entries.map(\.id))
                    }

                    // Invisible anchor used to auto-scroll to the latest entry
                    Color.clear
                        .frame(height: 1)
                        .id("live-timeline-bottom")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            }
            .onChange(of: model.entries.count) {
                withAnimation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82)) {
                    proxy.scrollTo("live-timeline-bottom", anchor: .bottom)
                }
            }
            .onAppear {
                model.update(
                    live: projection.liveTimeline,
                    persisted: projection.persistedTimeline,
                    xcode: projection.xcodeRuntimeObservations,
                    implementationCompletion: implementationCompletion,
                    generatedAt: projection.generatedAt
                )
            }
            .onChange(of: projection.generatedAt) { _, _ in
                model.update(
                    live: projection.liveTimeline,
                    persisted: projection.persistedTimeline,
                    xcode: projection.xcodeRuntimeObservations,
                    implementationCompletion: implementationCompletion,
                    generatedAt: projection.generatedAt
                )
            }
        }
        .frame(minWidth: 480, minHeight: 420)
        .accessibilityIdentifier("run-timeline-inspector-view")
    }

    private func entryIcon(for kind: FocusedTimelineSpineEntry.EntryKind) -> some View {
        let name: String = {
            switch kind {
            case .text: return "text.alignleft"
            case .mergedTool: return "hammer"
            case .sessionEvent: return "person.2.fill"
            case .agentSummary: return "doc.text.magnifyingglass"
            case .policyWarning: return "shield.lefthalf.filled"
            case .implementationCompletion: return "wrench.and.screwdriver"
            case .persisted: return "clock.fill"
            }
        }()
        return Image(systemName: name).font(.caption).foregroundStyle(.secondary)
    }
}

struct TimelineErrorPresentation: Equatable {
    let summary: String
    let highlights: [String]
    let shouldOfferRawDisclosure: Bool

    init(rawDetail: String) {
        let lower = rawDetail.lowercased()
        shouldOfferRawDisclosure = rawDetail.count > 120 || rawDetail.contains("\n")

        if lower.contains("xcode_mcp_initialize_timeout") {
            summary = "Xcode bridge initialization timed out"
            highlights = [
                "Check for an Xcode consent modal or a stalled simulator bridge, then retry the run.",
            ]
            return
        }

        if lower.contains("gemini cli command failed"), lower.contains("heap out of memory") {
            summary = "Gemini CLI failed: JavaScript heap out of memory."
            highlights = Self.relevantLines(
                in: rawDetail,
                matching: ["library/bluetooth", "out of memory", "heap limit"]
            )
            return
        }

        if let friendly = XcodeRuntimeFriendlyFailure.first(in: rawDetail) {
            summary = friendly.title
            highlights = [friendly.suggestedAction]
            return
        }

        summary = rawDetail
            .split(whereSeparator: \.isNewline)
            .first
            .map(String.init) ?? "Runtime error"
        highlights = []
    }

    private static func relevantLines(in raw: String, matching needles: [String]) -> [String] {
        raw.split(whereSeparator: \.isNewline)
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { line in
                let lower = line.lowercased()
                return needles.contains(where: lower.contains)
            }
    }
}

struct StreamingTimelineTextPresentation: Equatable {
    enum Kind: Equatable {
        case providerError
        case text
    }

    let kind: Kind
    let summary: String
    let shouldOfferRawDisclosure: Bool

    init(rawText: String) {
        let lower = rawText.lowercased()
        let looksLikeNodeStack = lower.contains("/node")
            || lower.contains("builtins_jsentrytrampoline")
            || lower.contains("node::")

        if looksLikeNodeStack {
            kind = .providerError
            summary = "Provider runtime error detected in Node stack output."
            shouldOfferRawDisclosure = true
        } else {
            kind = .text
            summary = rawText
                .split(whereSeparator: \.isNewline)
                .first
                .map(String.init) ?? ""
            shouldOfferRawDisclosure = rawText.count > 500 || rawText.contains("\n")
        }
    }
}

private struct XcodeRuntimeObservationsView: View {
    let observations: [WorkflowMapXcodeRuntimeObservation]

    var body: some View {
        GroupBox("Xcode Runtime") {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(observations) { observation in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(observation.stageLabel)
                            .font(.caption.weight(.semibold))
                        if let health = observation.brokerHealthLabel {
                            Text(health)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        if let simulator = observation.selectedSimulatorID {
                            Text(simulator)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                                .textSelection(.enabled)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .accessibilityIdentifier("xcode-runtime-observations")
    }
}

private struct TimelineEventDetailView: View {
    let event: ExecutionEvent

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(event.detail)
                .font(.caption)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
            if let toolName = event.toolName {
                Label(toolName, systemImage: "hammer")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
