import SwiftUI

struct FocusedTimelineSpineEntry: Identifiable, Sendable {
    let id: String
    let title: String
    let detail: String
    let timestamp: Date
    let stageID: String
    let surfaceLabel: String
    let sessionID: String?
    let liveEvent: ExecutionEvent?
}

func buildFocusedTimelineSpineEntries(
    liveTimeline: [LiveExecutionTimelineEntry],
    persistedTimeline: [WorkflowMapPersistedTimelineEntry],
    xcodeRuntimeObservations: [WorkflowMapXcodeRuntimeObservation] = []
) -> [FocusedTimelineSpineEntry] {
    let liveEntries = liveTimeline.map { entry in
        FocusedTimelineSpineEntry(
            id: entry.id.uuidString,
            title: entry.agentTitle,
            detail: entry.event.detail,
            timestamp: entry.event.timestamp,
            stageID: entry.stageID,
            surfaceLabel: entry.event.type.rawValue,
            sessionID: entry.event.sessionID,
            liveEvent: entry.event
        )
    }

    let persistedEntries = persistedTimeline.map { entry in
        FocusedTimelineSpineEntry(
            id: entry.id,
            title: entry.title,
            detail: entry.detail,
            timestamp: entry.timestamp,
            stageID: "persisted",
            surfaceLabel: "persisted",
            sessionID: entry.sessionID,
            liveEvent: nil
        )
    }

    let xcodePolicyWarnings = xcodeRuntimeObservations.flatMap { observation in
        observation.coalescedShimWarnings.enumerated().map { index, warning in
            FocusedTimelineSpineEntry(
                id: "\(observation.id)::policy-warning::\(index)",
                title: "Policy Warning",
                detail: "\(warning.policyReason): \(warning.matchedSubstring)",
                timestamp: warning.timestamp ?? Date(timeIntervalSince1970: 0),
                stageID: observation.stageID,
                surfaceLabel: "policy_warning",
                sessionID: nil,
                liveEvent: nil
            )
        }
    }

    return (liveEntries + persistedEntries + xcodePolicyWarnings).sorted { lhs, rhs in
        if lhs.timestamp == rhs.timestamp {
            return lhs.id > rhs.id
        }
        return lhs.timestamp > rhs.timestamp
    }
}

struct RunTimelineInspectorView: View {
    let projection: WorkflowMapProjection
    var showsTitle: Bool = true

    var body: some View {
        let timelineEntries = buildFocusedTimelineSpineEntries(
            liveTimeline: projection.liveTimeline,
            persistedTimeline: projection.persistedTimeline,
            xcodeRuntimeObservations: projection.xcodeRuntimeObservations
        )
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

                    if timelineEntries.isEmpty {
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
                                ForEach(timelineEntries) { entry in
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(entry.title)
                                                .font(.subheadline.weight(.semibold))
                                            Spacer()
                                            Text(entry.surfaceLabel)
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }

                                        if let liveEvent = entry.liveEvent {
                                            TimelineEventDetailView(event: liveEvent)
                                        } else if entry.surfaceLabel == "policy_warning" {
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
                        .animation(.spring(response: 0.45, dampingFraction: 0.82), value: timelineEntries.map(\.id))
                    }

                    // Invisible anchor used to auto-scroll to the latest entry
                    Color.clear
                        .frame(height: 1)
                        .id("live-timeline-bottom")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
            }
            .onChange(of: timelineEntries.count) {
                withAnimation(.spring(response: 0.45, dampingFraction: 0.82)) {
                    proxy.scrollTo("live-timeline-bottom", anchor: .bottom)
                }
            }
        }
        .frame(minWidth: 480, minHeight: 420)
        .accessibilityIdentifier("run-timeline-inspector-view")
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
