import Combine
import SwiftUI

@MainActor
final class P036TimelinePresentationModel: ObservableObject {
    @Published private(set) var entries: [FocusedTimelineSpineEntry] = []

    private var buffer: (live: [LiveExecutionTimelineEntry], persisted: [WorkflowMapPersistedTimelineEntry], xcode: [WorkflowMapXcodeRuntimeObservation])?
    private var lastFlush = Date.distantPast
    private let flushInterval: TimeInterval = 2.0
    private var timer: AnyCancellable?
    private var reduceMotion: Bool = false

    func update(
        live: [LiveExecutionTimelineEntry],
        persisted: [WorkflowMapPersistedTimelineEntry],
        xcode: [WorkflowMapXcodeRuntimeObservation],
        reduceMotion: Bool = false
    ) {
        buffer = (live, persisted, xcode)
        self.reduceMotion = reduceMotion

        let now = Date()
        if now.timeIntervalSince(lastFlush) >= flushInterval {
            flush()
        } else if timer == nil {
            timer = Timer.publish(every: 0.5, on: .main, in: .common)
                .autoconnect()
                .sink { [weak self] _ in
                    guard let self = self else { return }
                    if Date().timeIntervalSince(self.lastFlush) >= self.flushInterval {
                        self.flush()
                    }
                }
        }
    }

    private func flush() {
        timer?.cancel()
        timer = nil

        guard let buffer = buffer else { return }
        self.buffer = nil
        lastFlush = Date()

        let allEntries = buildFocusedTimelineSpineEntries(
            liveTimeline: buffer.live,
            persistedTimeline: buffer.persisted,
            xcodeRuntimeObservations: buffer.xcode
        )

        P036UICounters.shared.recordTimelineBatchFlush(
            entryCount: allEntries.count,
            reduceMotion: reduceMotion
        )
        entries = allEntries
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
    let providerID: String?
    let sessionID: String?
    let agentID: String?
    let isCollapsed: Bool
    let liveEvent: ExecutionEvent?

    init(
        id: String,
        kind: EntryKind,
        title: String,
        detail: String,
        timestamp: Date,
        stageID: String,
        surfaceLabel: String,
        providerID: String? = nil,
        sessionID: String?,
        agentID: String?,
        isCollapsed: Bool,
        liveEvent: ExecutionEvent?
    ) {
        self.id = id
        self.kind = kind
        self.title = title
        self.detail = detail
        self.timestamp = timestamp
        self.stageID = stageID
        self.surfaceLabel = surfaceLabel
        self.providerID = providerID
        self.sessionID = sessionID
        self.agentID = agentID
        self.isCollapsed = isCollapsed
        self.liveEvent = liveEvent
    }
    
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
    implementationCompletionTimestamp: Date? = nil
) -> [FocusedTimelineSpineEntry] {
    var rawEntries: [FocusedTimelineSpineEntry] = []
    
    if let completion = implementationCompletion, let timestamp = implementationCompletionTimestamp {
        let detail = [
            "\(completion.statusLabel): \(completion.outputFreshnessLabel)",
            completion.primaryEvidencePath
        ].compactMap { $0 }.joined(separator: " \u{00B7} ")
        
        rawEntries.append(FocusedTimelineSpineEntry(
            id: "implementation_completion",
            kind: .implementationCompletion,
            title: "Implementation Completion",
            detail: detail,
            timestamp: timestamp,
            stageID: "completion",
            surfaceLabel: "implementation_completion",
            sessionID: nil,
            agentID: nil,
            isCollapsed: false,
            liveEvent: nil
        ))
    }
    
    // Group live events by compound identity (agentID:sessionID:requestID) for reconciliation.
    // Keying on requestID alone allows concurrent agents sharing a requestID to erase each other.
    var toolCalls: [String: (start: LiveExecutionTimelineEntry, finish: LiveExecutionTimelineEntry?)] = [:]
    var otherLive: [LiveExecutionTimelineEntry] = []

    for entry in liveTimeline {
        if let requestId = entry.event.requestID {
            let compoundKey = "\(entry.agentID):\(entry.event.sessionID ?? ""):\(requestId)"
            if entry.event.type == .toolCallStarted {
                toolCalls[compoundKey] = (start: entry, finish: nil)
            } else if entry.event.type == .toolCallFinished {
                if var existing = toolCalls[compoundKey] {
                    existing.finish = entry
                    toolCalls[compoundKey] = existing
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

    // Pre-compute completedAgents before building any entries so merged tool cards
    // and other entries are collapsed in the same pass.
    var completedAgents = Set<String>()
    for entry in otherLive {
        if entry.event.type == .finalOutput || entry.event.type == .finish {
            completedAgents.insert(entry.agentID)
        }
    }

    // Collect all entries for the unified collapse pass.
    var entriesForCollapse: [FocusedTimelineSpineEntry] = []

    // Create merged tool entries — added to entriesForCollapse so the collapse pass
    // marks them isCollapsed when their agent has completed.
    for (compoundKey, call) in toolCalls {
        let isFinished = call.finish != nil
        entriesForCollapse.append(FocusedTimelineSpineEntry(
            id: compoundKey,
            kind: .mergedTool,
            title: call.start.agentTitle,
            detail: "Tool: \(call.start.event.toolName ?? "unknown") (\(isFinished ? "completed" : "running"))",
            timestamp: call.start.event.timestamp,
            stageID: call.start.stageID,
            surfaceLabel: "tool",
            sessionID: call.start.event.sessionID,
            agentID: call.start.agentID,
            isCollapsed: false,
            liveEvent: call.start.event
        ))
    }

    // Map other live entries. Do NOT skip duplicate summaries here;
    // the final reverse-chrono pass keeps the latest one per agent.
    for entry in otherLive {
        let kind: FocusedTimelineSpineEntry.EntryKind = {
            switch entry.event.type {
            case .textChunk: return .text
            case .sessionStarted, .sessionClosed: return .sessionEvent
            case .finalOutput, .finish: return .agentSummary
            case .toolCallFinished: return .sessionEvent // Diagnostic for out-of-order
            default: return .text
            }
        }()

        let title: String = {
            if entry.event.type == .toolCallFinished {
                return "Diagnostic: \(entry.agentTitle)"
            }
            return entry.agentTitle
        }()

        entriesForCollapse.append(FocusedTimelineSpineEntry(
            id: entry.id.uuidString,
            kind: kind,
            title: title,
            detail: entry.event.detail,
            timestamp: entry.event.timestamp,
            stageID: entry.stageID,
            surfaceLabel: entry.event.type.rawValue,
            sessionID: entry.event.sessionID,
            agentID: entry.agentID,
            isCollapsed: false,
            liveEvent: entry.event
        ))
    }

    // Collapse pass: text and merged-tool entries for completed agents are collapsed.
    for entry in entriesForCollapse {
        let shouldCollapse = (entry.kind == .text || entry.kind == .mergedTool) && completedAgents.contains(entry.agentID ?? "")
        rawEntries.append(FocusedTimelineSpineEntry(
            id: entry.id,
            kind: entry.kind,
            title: entry.title,
            detail: entry.detail,
            timestamp: entry.timestamp,
            stageID: entry.stageID,
            surfaceLabel: entry.surfaceLabel,
            sessionID: entry.sessionID,
            agentID: entry.agentID,
            isCollapsed: shouldCollapse,
            liveEvent: entry.liveEvent
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
            agentID: entry.agentID,
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
                agentID: observation.agentExecutionID.uuidString,
                isCollapsed: false,
                liveEvent: nil
            ))
        }
    }
    
    // Sorting and simple collapse rules (reverse chronological for top-down display)
    let sorted = rawEntries.sorted { lhs, rhs in
        if lhs.timestamp == rhs.timestamp {
            return lhs.id < rhs.id
        }
        return lhs.timestamp > rhs.timestamp
    }
    
    var finalEntries: [FocusedTimelineSpineEntry] = []
    var seenAgentsWithSummary = Set<String>()
    
    for entry in sorted {
        // one-summary-per-completed-agent logic
        if entry.kind == .agentSummary, let agentID = entry.agentID {
            if seenAgentsWithSummary.contains(agentID) {
                continue
            }
            seenAgentsWithSummary.insert(agentID)
        }

        if let last = finalEntries.last,
           last.kind == .text && entry.kind == .text,
           last.sessionID == entry.sessionID {
            // Collapse consecutive text from same session
            continue 
        }
        finalEntries.append(entry)
    }

    // Apply lossless 40-entry cap: preserve terminal/priority entries, fill remaining from normal.
    // sessionEvent covers terminal session events (started/closed) and diagnostic out-of-order
    // tool finishes — these must never be dropped per the P036 lossless policy.
    let cap = 40
    guard finalEntries.count > cap else { return finalEntries }
    let priority: Set<FocusedTimelineSpineEntry.EntryKind> = [
        .implementationCompletion, .agentSummary, .policyWarning, .sessionEvent, .persisted
    ]
    let reserved = finalEntries.filter { priority.contains($0.kind) }
    let normal = finalEntries.filter { !priority.contains($0.kind) }
    let remaining = max(0, cap - reserved.count)
    var capped = reserved + Array(normal.prefix(remaining))
    capped.sort { lhs, rhs in
        if lhs.timestamp == rhs.timestamp { return lhs.id < rhs.id }
        return lhs.timestamp > rhs.timestamp
    }
    return capped
}

struct RunTimelineInspectorView: View {
    let projection: WorkflowMapProjection
    var showsTitle: Bool = true
    
    @StateObject private var model = P036TimelinePresentationModel()
    @Environment(\.accessibilityReduceMotion) var reduceMotion

    private var isDogfood: Bool {
        #if DEBUG
        return true
        #else
        return ProcessInfo.processInfo.environment["CHAINWORKS_DOGFOOD"] == "1"
        #endif
    }

    var body: some View {
        Group {
            if !isDogfood {
                P031OperatorPlaceholder(
                    title: "Timeline Unavailable",
                    message: "Durable timeline requires dogfood flag activation.",
                    identifier: "timeline-dogfood-gated",
                    titleIdentifier: "timeline-dogfood-title"
                )
            } else {
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
                                            if !entry.isCollapsed {
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
                                                        if let agentID = entry.agentID {
                                                            Text(agentID)
                                                        }
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
                                                .transition(reduceMotion ? .opacity : .asymmetric(
                                                    insertion: .push(from: .bottom).combined(with: .opacity),
                                                    removal: .opacity
                                                ))
                                            }
                                        }
                                    }
                                }
                                .animation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82), value: model.entries.map(\.id))
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
                            reduceMotion: reduceMotion
                        )
                    }
                    .onChange(of: projection) {
                        model.update(
                            live: projection.liveTimeline,
                            persisted: projection.persistedTimeline,
                            xcode: projection.xcodeRuntimeObservations,
                            reduceMotion: reduceMotion
                        )
                    }
                }
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
            case .implementationCompletion: return "checkmark.seal.fill"
            case .persisted: return "clock.fill"
            }
        }()
        return Image(systemName: name).font(.caption).foregroundStyle(.secondary)
    }
}

private struct XcodeRuntimeObservationsView: View {
    let observations: [WorkflowMapXcodeRuntimeObservation]

    var body: some View {
        GroupBox("Xcode Runtime") {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(observations) { observation in
                    XcodeRuntimeObservationCard(observation: observation)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityIdentifier("xcode-runtime-section")
    }
}

private struct XcodeRuntimeObservationCard: View {
    let observation: WorkflowMapXcodeRuntimeObservation
    @State private var showAllWarnings = false

    private var broker: WorkflowMapXcodeBrokerObservation? {
        observation.latestBrokerObservation
    }

    private var coalescedWarnings: [WorkflowMapXcodeShimWarning] {
        observation.coalescedShimWarnings
    }

    private var visibleWarnings: [WorkflowMapXcodeShimWarning] {
        showAllWarnings ? coalescedWarnings : Array(coalescedWarnings.prefix(5))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(observation.agentTitle)
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Text(observation.stageLabel)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            if let broker {
                VStack(alignment: .leading, spacing: 6) {
                    runtimeRow("Lease", broker.leaseID ?? "none")
                        .accessibilityIdentifier("xcode-lease-id")
                    runtimeRow("Backend PID", broker.backendProcessID.map(String.init) ?? "none")
                        .accessibilityIdentifier("xcode-backend-pid")
                    if let brokerHealth = observation.brokerHealthLabel {
                        runtimeRow("Broker Health", brokerHealth)
                    }
                    runtimeRow("Start", broker.statusUpdate ?? broker.backendStartDisposition)
                    if let xcodePID = broker.xcodePID {
                        runtimeRow("Xcode PID", xcodePID)
                    }
                    if let home = broker.xcodeHomeDisposition {
                        runtimeRow("Host Home", home)
                    }
                    if let wait = broker.backendInitializeWaitMilliseconds {
                        runtimeRow("Initialize Wait", "\(wait) ms")
                    }
                    if let failure = broker.backendFailureClass,
                       let friendly = XcodeRuntimeFriendlyFailure.first(in: failure) {
                        friendlyFailure(friendly)
                    }
                }
            }

            if let simulatorID = observation.selectedSimulatorID {
                runtimeRow("Simulator", simulatorID)
            }

            if !observation.shimInvocations.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Shim Decisions")
                        .font(.caption.weight(.semibold))
                    ForEach(Array(observation.shimInvocations.enumerated()), id: \.offset) { _, invocation in
                        runtimeRow(invocation.tool, "\(invocation.policyDecision): \(invocation.policyReason)")
                    }
                }
            }

            if !observation.hostExecutorEvents.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Host Executor")
                        .font(.caption.weight(.semibold))
                    ForEach(Array(observation.hostExecutorEvents.enumerated()), id: \.offset) { _, event in
                        runtimeRow(event.tool, "\(event.hostEnvDisposition), exit \(event.exitStatus), \(event.durationMilliseconds) ms")
                    }
                }
            }

            if !coalescedWarnings.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(visibleWarnings.enumerated()), id: \.offset) { _, warning in
                        Label {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Policy Warning")
                                    .font(.caption.weight(.semibold))
                                Text("\(warning.policyReason): \(warning.matchedSubstring)")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .textSelection(.enabled)
                            }
                        } icon: {
                            Image(systemName: "exclamationmark.shield")
                        }
                        .foregroundStyle(.orange)
                        .accessibilityIdentifier("xcode-policy-warning")
                    }

                    if coalescedWarnings.count > 5 {
                        DisclosureGroup(
                            showAllWarnings ? "Hide residual paths" : "View all residual paths",
                            isExpanded: $showAllWarnings
                        ) {
                            EmptyView()
                        }
                        .font(.caption)
                    }
                }
            }

            if observation.storage.truncated {
                Text("Observation truncated after dropping \(observation.storage.totalEventsDropped) events.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(10)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private func runtimeRow(_ label: String, _ value: String) -> some View {
        LabeledContent(label) {
            Text(value)
                .font(.caption.monospaced())
                .textSelection(.enabled)
        }
        .font(.caption)
    }

    private func friendlyFailure(_ failure: XcodeRuntimeFriendlyFailure) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(failure.title, systemImage: "exclamationmark.triangle.fill")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.orange)
            Text(failure.suggestedAction)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
        }
        .accessibilityIdentifier("xcode-friendly-failure")
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
        if let xcodeFailure = XcodeRuntimeFriendlyFailure.first(in: rawDetail) {
            summary = xcodeFailure.title
            highlights = [xcodeFailure.suggestedAction]
            self.rawDetail = rawDetail
            return
        }

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

struct XcodeRuntimeFriendlyFailure: Equatable, Sendable {
    let failureClass: String
    let title: String
    let suggestedAction: String

    static func first(in text: String) -> XcodeRuntimeFriendlyFailure? {
        let lowercased = text.lowercased()
        return all.first { failure in
            lowercased.contains(failure.failureClass)
        }
    }

    private static let all: [XcodeRuntimeFriendlyFailure] = [
        .init(
            failureClass: "provider_http_mcp_unsupported",
            title: "Provider does not support HTTP MCP",
            suggestedAction: "Use Codex ACP, Claude Agent ACP, or Gemini CLI with a verified HTTP MCP version."
        ),
        .init(
            failureClass: "xcode_mcp_registry_stale_stdio",
            title: "Xcode MCP registry entry uses direct stdio",
            suggestedAction: "Update the machine MCP registry to the brokered Xcode entry or remove stale xcrun mcpbridge command fields."
        ),
        .init(
            failureClass: "xcode_mcp_registry_ambiguous",
            title: "Multiple Xcode MCP registry entries match",
            suggestedAction: "Keep one canonical xcode broker entry and remove duplicate or ambiguous entries."
        ),
        .init(
            failureClass: "host_env_unavailable",
            title: "Host Xcode environment unavailable",
            suggestedAction: "Confirm the daemon runs as the GUI user or configure the operator-home override, then retry the step."
        ),
        .init(
            failureClass: "pool_pid_drift",
            title: "Xcode process changed during run",
            suggestedAction: "Ensure the intended Xcode workspace is open and retry the failed execution."
        ),
        .init(
            failureClass: "xcode_mcp_capacity_exhausted",
            title: "Xcode bridge capacity reached",
            suggestedAction: "Wait for active Xcode reviewers to finish, reduce fan-out, or raise the runtime-profile limit deliberately."
        ),
        .init(
            failureClass: "xcode_mcp_initialize_timeout",
            title: "Xcode bridge initialization timed out",
            suggestedAction: "Check for an Xcode consent modal, confirm Xcode is responsive, and retry."
        ),
        .init(
            failureClass: "xcode_mcp_action_required",
            title: "Check Xcode to continue",
            suggestedAction: "Bring Xcode to the foreground and respond to any consent or authorization prompt."
        ),
        .init(
            failureClass: "xcode_mcp_first_connect_timeout",
            title: "Provider did not connect to Xcode bridge",
            suggestedAction: "Retry the execution; if repeated, inspect provider HTTP MCP support and session/new payload logs."
        ),
        .init(
            failureClass: "xcode_shim_no_active_prompt",
            title: "Xcode command ran outside the active prompt",
            suggestedAction: "Retry the execution; if repeated, disable session reuse for that agent or inspect background shell activity."
        ),
        .init(
            failureClass: "simulator_destination_ambiguous",
            title: "Simulator destination is ambiguous",
            suggestedAction: "Choose one of the listed simulator UUIDs or remove duplicate simulator name/OS matches."
        ),
        .init(
            failureClass: "xcode_build_concurrency_contention",
            title: "Xcode build resources are busy",
            suggestedAction: "Retry after sibling builds finish, use a different DerivedData path, or reduce parallel build fan-out for this workflow."
        ),
        .init(
            failureClass: "xcode_target_not_found",
            title: "Xcode target was not found",
            suggestedAction: "Open the intended workspace in Xcode and retry the execution."
        ),
        .init(
            failureClass: "xcode_target_ambiguous",
            title: "Multiple Xcode targets match",
            suggestedAction: "Select one Xcode PID or workspace explicitly before retrying."
        )
    ]
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

    nonisolated private static func compactLine(_ line: String) -> String {
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
