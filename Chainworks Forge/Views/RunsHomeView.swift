import SwiftUI
import Combine
#if os(macOS)
import AppKit
#endif

struct RunsHomeView: View {
    @StateObject private var model: P031ThinReadDashboardModel
    // P046: transient MainActor session observability state; never persisted to SwiftData.
    // Owned here as the selected-run detail coordinator per Phase 3 requirement.
    @StateObject private var p046Model: P046SessionObservabilityModel
    @ObservedObject var workbench: RunsWorkbenchPresentationModel
    @State private var selectedRunDetailTab: P031RunDetailTab = .overview
    @State private var focusedArtifactStageID: String?
    @State private var closeoutReadinessScrollRequest = 0
    @FocusState private var closeoutReadinessFocus: P077CloseoutReadinessFocus?
    // PC-003: sidebar lane filter context — set when a deep-link or banner routes here
    // specifically to surface waiting approvals. Cleared when the user manually selects any run.
    @State private var focusedLaneID: String? = nil

    @MainActor
    init(workbench: RunsWorkbenchPresentationModel) {
        let model = P031ThinReadDashboardModel.bootstrap()
        // Share the same endpoint for P046 session observability reads.
        let endpoint = DaemonClientEndpoint.operatorDefault()
        let p046Store = P031GraphQLWorkflowReadStore(
            readTransport: P031URLSessionGraphQLReadTransport(endpoint: endpoint),
            subscriptionTransport: P031URLSessionGraphQLSubscriptionTransport(endpoint: endpoint)
        )
        _model = StateObject(wrappedValue: model)
        _p046Model = StateObject(wrappedValue: P046SessionObservabilityModel.make(store: p046Store))
        self.workbench = workbench
        _selectedRunDetailTab = State(initialValue: .overview)
    }

    @MainActor
    init(
        model: P031ThinReadDashboardModel,
        workbench: RunsWorkbenchPresentationModel,
        initialTab: P031RunDetailTab,
        p046Model: P046SessionObservabilityModel? = nil
    ) {
        _model = StateObject(wrappedValue: model)
        _p046Model = StateObject(wrappedValue: p046Model ?? P046SessionObservabilityModel.noOp())
        self.workbench = workbench
        _selectedRunDetailTab = State(initialValue: initialTab)
    }

    var body: some View {
        NavigationSplitView {
            runsSidebar
                .navigationSplitViewColumnWidth(min: 280, ideal: 320)
        } detail: {
            runDetailPane
        }
        .accessibilityIdentifier("runs-home-owner-view")
        .task {
            await model.loadIfNeeded()
        }
        // P046: drive session observability for the currently selected run.
        // Capability discovery and gating happen inside the model before any P046
        // documents are issued. On run change the prior task is cancelled automatically
        // by SwiftUI's .task(id:) semantics; here we use onChange for the same effect.
        .onChange(of: model.selectedRunID) { _, newRunID in
            p046Model.updateSelectedRun(newRunID)
        }
        .toolbar {
            Button {
                Task { await model.refreshAll() }
            } label: {
                if model.isLoading {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
            }
            .disabled(model.isLoading)
        }
        .onReceive(NotificationCenter.default.publisher(for: Notification.Name("chainworks.selectRunDetailTab"))) { notification in
            guard
                let userInfo = notification.userInfo,
                let rawValue = userInfo["tab"] as? String,
                let tab = P031RunDetailTab(rawValue: rawValue)
            else {
                #if DEBUG
                if let rawValue = notification.userInfo?["tab"] as? String {
                    print("[P036] selectRunDetailTab: unknown rawValue '\(rawValue)'")
                }
                #endif
                return
            }
            selectedRunDetailTab = tab
        }
    }

    private func runRow(row: P031RunsHomeRowPresentation) -> some View {
        Button {
            selectedRunDetailTab = .overview
            focusedArtifactStageID = nil
            focusedLaneID = nil  // PC-003: clear filter context on manual selection
            model.selectRun(row.runID)
        } label: {
            P031RunsHomeRowCard(
                row: row,
                isSelected: model.selectedRunID == row.runID
            )
        }
        .buttonStyle(.plain)
        .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
    }

    // PC-003: extracted to break up the type-checker load in runsSidebar body.
    @ViewBuilder
    private func laneSectionHeader(for lane: RunsWorkbenchPresentationModel.SidebarLane) -> some View {
        if focusedLaneID == lane.id {
            Label(lane.title, systemImage: "scope")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.accentColor)
                .accessibilityIdentifier("lane-focused-\(lane.id)")
        } else {
            Text(lane.title)
        }
    }

    private var runsSidebar: some View {
        List {
            if workbench.sidebarLanes.isEmpty {
                Section {
                    if model.isLoading {
                        VStack(alignment: .leading, spacing: 8) {
                            ForgeSkeleton.text(width: 120)
                            ForgeSkeleton.text(width: 180)
                            ForgeSkeleton.text(width: 140)
                        }
                        .padding(.vertical, 8)
                    } else {
                        ForgeEmptyState(
                            title: model.runsHome?.emptyStateTitle ?? "No runs",
                            systemImage: "house",
                            description: model.runsHome?.errorDescription ?? model.runsHome?.refreshFeedbackText ?? ""
                        )
                    }
                } header: {
                    Text("Runs")
                }
            } else {
                ForEach(workbench.sidebarLanes) { lane in
                    Section {
                        ForEach(lane.runs, id: \.runID) { row in
                            runRow(row: row)
                        }
                    } header: {
                        laneSectionHeader(for: lane)
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .accessibilityIdentifier("runs-home-list")
        .onReceive(model.$runDetail.compactMap { $0 }) { newValue in
            workbench.populate(from: newValue)
        }
        .onReceive(model.$approvalInbox.compactMap { $0 }) { newValue in
            workbench.populate(from: newValue)
        }
        .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenRunInRunsHome)) { notification in
            if let runID = notification.object as? String {
                selectedRunDetailTab = .overview
                focusedArtifactStageID = nil
                model.selectRun(runID)
            }
        }
        // PC-003: approvals deep-link → Runs focused on waiting approval lane.
        // Always set focusedLaneID so the sidebar shows the filter context even when
        // no run is currently waiting (the empty-state message remains informative).
        // focusedLaneID is kept set so the sidebar-lane publisher below can
        // replay the selection once lanes populate (fixes the startup notification race).
        .onReceive(NotificationCenter.default.publisher(for: .chainworksFocusWaitingApprovalLane)) { _ in
            focusedLaneID = "waiting"
            if let waitingLane = workbench.sidebarLanes.first(where: { $0.id == "waiting" }),
               let firstRun = waitingLane.runs.first {
                selectedRunDetailTab = .approvals
                model.selectRun(firstRun.runID)
            }
        }
        // PC-003 workbench-flag race fix: ContentView sets pendingFocusWaitingApprovalLane before
        // switching tabs so the flag is present when RunsHomeView mounts. initial:true fires on
        // first render covering the tab-switch case where the notification fires before the view
        // is in the hierarchy. The handler sets focusedLaneID so the lanes-change fallback below
        // still auto-selects if lanes haven't loaded yet when this fires.
        .onChange(of: workbench.pendingFocusWaitingApprovalLane, initial: true) {
            guard workbench.pendingFocusWaitingApprovalLane else { return }
            workbench.clearFocusWaitingApprovalLane()
            focusedLaneID = "waiting"
            if let waitingLane = workbench.sidebarLanes.first(where: { $0.id == "waiting" }),
               let firstRun = waitingLane.runs.first {
                selectedRunDetailTab = .approvals
                model.selectRun(firstRun.runID)
            }
        }
        // PC-003 lanes-change fallback: if focusedLaneID was set (by notification or workbench flag)
        // before lanes populated, auto-select once lanes arrive.
        .onReceive(workbench.$sidebarLanes) { sidebarLanes in
            guard focusedLaneID == "waiting", model.selectedRunID == nil else { return }
            if let waitingLane = sidebarLanes.first(where: { $0.id == "waiting" }),
               let firstRun = waitingLane.runs.first {
                selectedRunDetailTab = .approvals
                model.selectRun(firstRun.runID)
            }
        }
    }

    private var runDetailPane: some View {
        VStack(alignment: .leading, spacing: 14) {
            runDetailAlert

            if let runDetail = model.runDetail {
                Picker("Run section", selection: $selectedRunDetailTab) {
                    ForEach(P031RunDetailTab.allCases) { tab in
                        Text(tab.title).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(maxWidth: 720)
                .padding(.horizontal, 20)
                .padding(.top, 20)

                runDetailTabContent(runDetail)
            } else {
                ScrollView {
                    P031CalloutCard(
                        title: "Run detail unavailable",
                        bodyText: model.runsHome?.emptyStateTitle ?? "Select a run to inspect server projections.",
                        accentColor: .secondary
                    )
                    .padding(20)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(nsColor: .windowBackgroundColor))
        .accessibilityIdentifier("run-detail-panel")
    }

    @ViewBuilder
    private var runDetailAlert: some View {
        if let message = model.daemonSchemaMismatchMessage {
            P031DaemonUpdateRequiredCard(
                title: "Daemon schema mismatch",
                message: message,
                restartError: model.daemonRestartError,
                isRestarting: model.isRestartingDaemon,
                onRestart: {
                    Task { await model.restartDaemonForUpdateRequired() }
                }
            )
            .padding(.horizontal, 20)
            .padding(.top, 20)
        } else if let message = model.daemonBuildMismatchMessage {
            P031DaemonUpdateRequiredCard(
                title: "Daemon update required",
                message: message,
                restartError: model.daemonRestartError,
                isRestarting: model.isRestartingDaemon,
                onRestart: {
                    Task { await model.restartDaemonForUpdateRequired() }
                }
            )
            .padding(.horizontal, 20)
            .padding(.top, 20)
        } else if let daemonLifecycle = model.daemonLifecycle,
                  daemonLifecycle.state == nil,
                  daemonLifecycle.title == "Daemon unavailable" {
            P031CalloutCard(
                title: daemonLifecycle.title,
                bodyText: daemonLifecycle.errorDescription ?? daemonLifecycle.refreshFeedbackText,
                accentColor: .orange
            ) {
                P031FreshnessBadge(snapshot: daemonLifecycle.freshness)
            }
            .padding(.horizontal, 20)
            .padding(.top, 20)
            .accessibilityIdentifier("p031-daemon-unavailable-alert-\(daemonLifecycle.freshness.state.rawValue)")
        }
    }

    @ViewBuilder
    private func runDetailTabContent(_ runDetail: P031RunDetailPresentation) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    switch selectedRunDetailTab {
                    case .overview:
                        if let header = workbench.summaryHeader {
                            P036RunDetailSummaryCard(
                                header: header,
                                onCheckSystemReadiness: {
                                    NotificationCenter.default.post(
                                        name: .chainworksOpenSystemReadiness,
                                        object: nil
                                    )
                                }
                            )
                        }

                        if !workbench.inlineApprovals.isEmpty {
                            P036ApprovalWorkbenchCard(
                                rows: workbench.inlineApprovals,
                                onApprove: { id in await model.settleApproval(id, action: .approve) },
                                onReject: { id in await model.settleApproval(id, action: .reject(reason: "inline_ui_reject")) },
                                resolvingIDs: model.resolvingApprovalIDs
                            )
                        }

                        if let stageMap = workbench.stageMap {
                            P036StageMapCard(map: stageMap)
                        }

                        if let continuationReadback = runDetail.continuationReadback {
                            P086ContinuationReadbackCard(presentation: continuationReadback)
                        }

                        if !workbench.artifactsAndReports.isEmpty {
                            P036ArtifactWorkbenchCard(rows: workbench.artifactsAndReports)
                        }

                        if !workbench.recoveryEvidence.isEmpty {
                            P036RecoveryEvidenceCard(rows: workbench.recoveryEvidence)
                        }

                        if let health = workbench.freshnessAndHealth {
                            P036SystemReadinessCard(health: health)
                        }

                        P046SessionObservabilityCard(model: p046Model)

                    case .stages:
                        if let stageMap = workbench.stageMap {
                            P036StageMapCard(map: stageMap)
                        }
                    case .artifacts:
                        P031ArtifactViewerCard(
                            rows: runDetail.artifactViewerRows,
                            focusedStageID: focusedArtifactStageID,
                            loadArtifactPreview: { id in await model.loadArtifactPreview(artifactID: id) }
                        )
                    case .approvals:
                        P036ApprovalWorkbenchCard(
                            rows: workbench.inlineApprovals,
                            onApprove: { id in await model.settleApproval(id, action: .approve) },
                            onReject: { id in await model.settleApproval(id, action: .reject(reason: "inline_ui_reject")) },
                            resolvingIDs: model.resolvingApprovalIDs
                        )
                    case .timeline:
                        P036TimelineWorkbenchCard(
                            entries: timelineEntriesForSelectedRun(),
                            activeAgents: workbench.activeTimelineAgents,
                            resolveTimelineRawDetail: { handle in
                                await model.resolveTimelineRawDetail(handle: handle)
                            }
                        )
                    case .reports:
                        P031ReportMetadataCard(rows: workbench.reportRows)
                    case .system:
                        if let health = workbench.freshnessAndHealth {
                            P036SystemReadinessCard(health: health)
                        }
                    }
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 20)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .onChange(of: closeoutReadinessScrollRequest) {
                guard selectedRunDetailTab == .overview else { return }
                withAnimation(.easeInOut(duration: 0.16)) {
                    proxy.scrollTo(P077CloseoutReadinessAnchor.card, anchor: .top)
                }
            }
        }
    }

    private func activateCloseoutReadinessFromCompactSignal() {
        selectedRunDetailTab = .overview
        DispatchQueue.main.async {
            closeoutReadinessScrollRequest += 1
        }
    }

    private func focusCloseoutPrimaryUnblock() {
        selectedRunDetailTab = .overview
        DispatchQueue.main.async {
            closeoutReadinessScrollRequest += 1
        }
    }

    private func timelineEntriesForSelectedRun() -> [RunsWorkbenchPresentationModel.TimelineEntry] {
        let liveEntries = model.runtimeTimelineEvents.map { event in
            RunsWorkbenchPresentationModel.TimelineEntry(
                id: event.id,
                kind: RunsWorkbenchPresentationModel.TimelineEntryKind(rawValue: event.surfaceLabel) ?? .sessionEvent,
                title: event.title,
                detail: event.detail,
                timestamp: event.timestamp,
                displayTime: event.timestamp.formatted(date: .omitted, time: .standard),
                stageID: event.stageID,
                surfaceLabel: event.surfaceLabel,
                agentID: event.agentID,
                sessionID: event.sessionGenerationID,
                isCollapsed: false,
                rawDetail: event.rawDetail,
                rawDetailBytes: event.rawDetailBytes,
                rawDetailTruncated: event.rawDetailTruncated,
                rawDetailHandle: event.rawDetailHandle,
                rawDetailDigest: event.rawDetailDigest,
                fullRawAvailable: event.fullRawAvailable,
                detailDigest: event.detailDigest,
                detailCharCount: event.detailCharCount,
                chunkCount: event.chunkCount,
                isStreaming: event.isStreaming,
                isTerminal: event.isTerminal,
                stateLabel: event.stateLabel,
                providerID: event.provider
            )
        }
        return liveEntries.isEmpty ? workbench.timelineEntries : liveEntries
    }

    private func artifactCountsByStageID(
        for runDetail: P031RunDetailPresentation
    ) -> [String: Int] {
        Dictionary(
            grouping: runDetail.artifactViewerRows,
            by: { $0.stageExecutionID ?? $0.stageID }
        )
        .mapValues { $0.count }
    }
}

struct P036RuntimeTimelineBuffer {
    private static let liveResponseDetailLimit = 64_000
    private static let retainedRawResponseDetailLimitBytes = 512 * 1024

    private(set) var events: [P031RuntimeTimelineEventPresentation] = []

    mutating func reset() {
        events = []
    }

    mutating func record(
        _ event: P031RuntimeTimelineEventPresentation,
        selectedRunID: String?
    ) {
        guard selectedRunID == event.runID else { return }

        if event.surfaceLabel == "final_response" {
            collapseResponseChunks(
                matching: event,
                fallbackTerminalEvent: event,
                terminalDetailOverride: event.detail
            )
            return
        }

        if event.eventKind == "session_completed" || event.eventKind == "session_failed" {
            collapseResponseChunks(
                matching: event,
                fallbackTerminalEvent: event,
                terminalDetailOverride: nil
            )
            return
        }

        if Self.isResponseChunk(event) {
            mergeResponseChunk(event)
            return
        }

        if Self.isProviderActionCompletion(event) {
            collapseProviderActionCompletion(event)
            return
        }

        append(event)
    }

    private mutating func append(_ event: P031RuntimeTimelineEventPresentation) {
        events.append(event)
        trimToVisibleLimit()
    }

    private mutating func trimToVisibleLimit() {
        while events.count > 40 {
            if let removableIndex = events.indices.first(where: { !Self.isResponseChunk(events[$0]) }) {
                events.remove(at: removableIndex)
            } else {
                events.removeFirst()
            }
        }
    }

    private mutating func mergeResponseChunk(_ event: P031RuntimeTimelineEventPresentation) {
        guard let existingIndex = events.indices.reversed().first(where: { index in
            let existing = events[index]
            return Self.isResponseChunk(existing)
                && Self.matchesAgentSession(existing, terminalEvent: event)
        })
        else {
            append(Self.normalizedResponseChunk(event))
            return
        }

        let previous = events.remove(at: existingIndex)
        let previousRawDetail = previous.rawDetail ?? previous.detail
        let incomingRawDetail = event.rawDetail ?? event.detail
        let combinedRawDetail = previousRawDetail + incomingRawDetail
        let rawDetailWasTruncated = previous.rawDetailTruncated
            || event.rawDetailTruncated
            || combinedRawDetail.utf8.count > Self.retainedRawResponseDetailLimitBytes
        let rawDetail = Self.boundedRawResponseDetail(combinedRawDetail)
        let rawDetailBytes = Self.combinedRawDetailBytes(previous.rawDetailBytes, event.rawDetailBytes)
        let rawDetailHandle = event.rawDetailHandle ?? previous.rawDetailHandle
        append(P031RuntimeTimelineEventPresentation(
            id: previous.id,
            runID: event.runID,
            stageID: event.stageID ?? previous.stageID,
            agentID: event.agentID,
            provider: event.provider,
            eventKind: event.eventKind,
            title: event.title,
            detail: Self.boundedLiveResponseDetail(rawDetail),
            surfaceLabel: event.surfaceLabel,
            sessionGenerationID: previous.sessionGenerationID ?? event.sessionGenerationID,
            timestamp: event.timestamp,
            rawDetail: rawDetail,
            rawDetailBytes: rawDetailBytes,
            rawDetailTruncated: rawDetailWasTruncated,
            rawDetailHandle: rawDetailHandle,
            rawDetailDigest: event.rawDetailDigest ?? previous.rawDetailDigest,
            fullRawAvailable: !rawDetailWasTruncated
                || (rawDetailHandle != nil && event.fullRawAvailable && previous.fullRawAvailable),
            detailDigest: event.detailDigest ?? previous.detailDigest,
            detailCharCount: rawDetail.count,
            chunkCount: (previous.chunkCount ?? 1) + (event.chunkCount ?? 1),
            isStreaming: true,
            isTerminal: false,
            stateLabel: event.stateLabel ?? previous.stateLabel
        ))
    }

    private mutating func collapseProviderActionCompletion(_ event: P031RuntimeTimelineEventPresentation) {
        guard let incomingIdentity = Self.providerActionIdentity(for: event),
              let existingIndex = events.indices.reversed().first(where: { index in
                  let existing = events[index]
                  return Self.isProviderActionInProgress(existing)
                      && Self.matchesAgentSession(existing, terminalEvent: event)
                      && Self.providerActionIdentity(for: existing) == incomingIdentity
              })
        else {
            append(event)
            return
        }

        let previous = events.remove(at: existingIndex)
        append(P031RuntimeTimelineEventPresentation(
            id: event.id,
            runID: event.runID,
            stageID: event.stageID ?? previous.stageID,
            agentID: event.agentID,
            provider: event.provider,
            eventKind: event.eventKind,
            title: event.title,
            detail: event.detail,
            surfaceLabel: event.surfaceLabel,
            sessionGenerationID: event.sessionGenerationID ?? previous.sessionGenerationID,
            timestamp: event.timestamp,
            rawDetail: event.rawDetail ?? event.detail,
            rawDetailBytes: event.rawDetailBytes,
            rawDetailTruncated: event.rawDetailTruncated,
            rawDetailHandle: event.rawDetailHandle,
            rawDetailDigest: event.rawDetailDigest,
            fullRawAvailable: event.fullRawAvailable,
            detailDigest: event.detailDigest,
            detailCharCount: event.detailCharCount,
            chunkCount: event.chunkCount,
            isStreaming: false,
            isTerminal: true,
            stateLabel: event.stateLabel ?? previous.stateLabel
        ))
    }

    private mutating func collapseResponseChunks(
        matching terminalEvent: P031RuntimeTimelineEventPresentation,
        fallbackTerminalEvent: P031RuntimeTimelineEventPresentation,
        terminalDetailOverride: String?
    ) {
        let matchingChunks = events.filter { existing in
            Self.isResponseChunk(existing)
                && Self.matchesAgentSession(existing, terminalEvent: terminalEvent)
        }
        let rawDetail = Self.accumulatedResponseDetail(
            for: matchingChunks,
            override: terminalDetailOverride
        )
        guard !rawDetail.isEmpty else { return }
        let summaryDetail = Self.summaryDetail(
            for: matchingChunks,
            rawDetail: rawDetail
        )
        let rawDetailWasTruncated = matchingChunks.contains(where: \.rawDetailTruncated)
            || rawDetail.utf8.count >= Self.retainedRawResponseDetailLimitBytes
        let rawDetailBytes = Self.accumulatedRawDetailBytes(
            for: matchingChunks,
            fallback: fallbackTerminalEvent
        )

        events.removeAll { existing in
            Self.isResponseChunk(existing)
                && Self.matchesAgentSession(existing, terminalEvent: terminalEvent)
        }

        let lastChunk = matchingChunks.last ?? fallbackTerminalEvent
        append(
            P031RuntimeTimelineEventPresentation(
                id: "\(fallbackTerminalEvent.id):response-summary",
                runID: fallbackTerminalEvent.runID,
                stageID: lastChunk.stageID ?? fallbackTerminalEvent.stageID,
                agentID: fallbackTerminalEvent.agentID,
                provider: fallbackTerminalEvent.provider,
                eventKind: fallbackTerminalEvent.eventKind,
                title: fallbackTerminalEvent.eventKind == "session_failed"
                    ? "Agent response failed"
                    : "Agent response complete",
                detail: summaryDetail,
                surfaceLabel: "agent_summary",
                sessionGenerationID: lastChunk.sessionGenerationID,
                timestamp: fallbackTerminalEvent.timestamp,
                rawDetail: rawDetail,
                rawDetailBytes: rawDetailBytes,
                rawDetailTruncated: rawDetailWasTruncated,
                rawDetailHandle: lastChunk.rawDetailHandle ?? fallbackTerminalEvent.rawDetailHandle,
                rawDetailDigest: lastChunk.rawDetailDigest ?? fallbackTerminalEvent.rawDetailDigest,
                fullRawAvailable: !rawDetailWasTruncated
                    || ((lastChunk.rawDetailHandle ?? fallbackTerminalEvent.rawDetailHandle) != nil
                        && lastChunk.fullRawAvailable
                        && fallbackTerminalEvent.fullRawAvailable),
                detailDigest: fallbackTerminalEvent.detailDigest,
                detailCharCount: rawDetail.count,
                chunkCount: matchingChunks.reduce(0) { total, chunk in
                    total + (chunk.chunkCount ?? 1)
                },
                isStreaming: false,
                isTerminal: true,
                stateLabel: fallbackTerminalEvent.stateLabel ?? lastChunk.stateLabel
            )
        )
    }

    private static func isResponseChunk(_ event: P031RuntimeTimelineEventPresentation) -> Bool {
        event.surfaceLabel == "agent_message_chunk" || event.surfaceLabel == "text_chunk"
    }

    private static func isProviderAction(_ event: P031RuntimeTimelineEventPresentation) -> Bool {
        event.surfaceLabel == "provider_activity"
            || event.surfaceLabel == "tool_call"
            || event.surfaceLabel == "tool_call_update"
    }

    private static func isProviderActionInProgress(_ event: P031RuntimeTimelineEventPresentation) -> Bool {
        isProviderAction(event) && event.detail.localizedCaseInsensitiveContains("in_progress")
    }

    private static func isProviderActionCompletion(_ event: P031RuntimeTimelineEventPresentation) -> Bool {
        isProviderAction(event) && event.detail.localizedCaseInsensitiveContains("completed")
    }

    private static func providerActionIdentity(for event: P031RuntimeTimelineEventPresentation) -> String? {
        let tokens = event.detail.components(separatedBy: CharacterSet.whitespacesAndNewlines.union(
            CharacterSet(charactersIn: "·'\"`()[]{}")
        ))
        let pathTokens = tokens.compactMap { rawToken -> String? in
            let token = rawToken.trimmingCharacters(in: CharacterSet(charactersIn: ",;:"))
            guard token.contains("/") || token.contains(".") else { return nil }
            return token.split(separator: "/").last.map(String.init)
        }
        if let lastPath = pathTokens.last, !lastPath.isEmpty {
            return lastPath
        }

        let normalized = event.detail
            .replacingOccurrences(of: "in_progress", with: "", options: .caseInsensitive)
            .replacingOccurrences(of: "completed", with: "", options: .caseInsensitive)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return normalized.isEmpty ? nil : normalized
    }

    private static func combinedRawDetailBytes(_ lhs: Int?, _ rhs: Int?) -> Int? {
        guard let lhs, let rhs else { return nil }
        return boundedRawDetailBytes(lhs + rhs)
    }

    private static func accumulatedRawDetailBytes(
        for chunks: [P031RuntimeTimelineEventPresentation],
        fallback: P031RuntimeTimelineEventPresentation
    ) -> Int? {
        guard !chunks.isEmpty else {
            return boundedRawDetailBytes(fallback.rawDetailBytes)
        }
        var total = 0
        for chunk in chunks {
            guard let rawDetailBytes = chunk.rawDetailBytes else {
                return nil
            }
            total += rawDetailBytes
        }
        return boundedRawDetailBytes(total)
    }

    private static func boundedRawDetailBytes(_ bytes: Int?) -> Int? {
        bytes.map { min($0, retainedRawResponseDetailLimitBytes) }
    }

    private static func normalizedResponseChunk(
        _ event: P031RuntimeTimelineEventPresentation
    ) -> P031RuntimeTimelineEventPresentation {
        let incomingRawDetail = event.rawDetail ?? event.detail
        let rawDetailWasTruncated = event.rawDetailTruncated
            || incomingRawDetail.utf8.count > Self.retainedRawResponseDetailLimitBytes
        let rawDetail = Self.boundedRawResponseDetail(incomingRawDetail)
        return P031RuntimeTimelineEventPresentation(
            id: event.id,
            runID: event.runID,
            stageID: event.stageID,
            agentID: event.agentID,
            provider: event.provider,
            eventKind: event.eventKind,
            title: event.title,
            detail: Self.boundedLiveResponseDetail(rawDetail),
            surfaceLabel: event.surfaceLabel,
            sessionGenerationID: event.sessionGenerationID,
            timestamp: event.timestamp,
            rawDetail: rawDetail,
            rawDetailBytes: Self.boundedRawDetailBytes(event.rawDetailBytes),
            rawDetailTruncated: rawDetailWasTruncated,
            rawDetailHandle: event.rawDetailHandle,
            rawDetailDigest: event.rawDetailDigest,
            fullRawAvailable: !rawDetailWasTruncated
                || (event.rawDetailHandle != nil && event.fullRawAvailable),
            detailDigest: event.detailDigest,
            detailCharCount: rawDetail.count,
            chunkCount: event.chunkCount ?? 1,
            isStreaming: true,
            isTerminal: false,
            stateLabel: event.stateLabel
        )
    }

    private static func matchesAgentSession(
        _ event: P031RuntimeTimelineEventPresentation,
        terminalEvent: P031RuntimeTimelineEventPresentation
    ) -> Bool {
        guard event.agentID == terminalEvent.agentID else { return false }
        guard event.runID == terminalEvent.runID else { return false }
        if let terminalSessionID = terminalEvent.sessionGenerationID {
            return event.sessionGenerationID == terminalSessionID
        }
        return true
    }

    private static func boundedLiveResponseDetail(_ detail: String) -> String {
        guard detail.count > liveResponseDetailLimit else { return detail }
        return String(detail.suffix(liveResponseDetailLimit))
    }

    private static func boundedRawResponseDetail(_ detail: String) -> String {
        guard detail.utf8.count > retainedRawResponseDetailLimitBytes else { return detail }

        var retained = ""
        retained.reserveCapacity(min(detail.count, retainedRawResponseDetailLimitBytes))
        for character in detail.reversed() {
            let next = String(character)
            if retained.utf8.count + next.utf8.count > retainedRawResponseDetailLimitBytes {
                break
            }
            retained.insert(character, at: retained.startIndex)
        }
        return retained
    }

    private static func accumulatedResponseDetail(
        for chunks: [P031RuntimeTimelineEventPresentation],
        override: String?
    ) -> String {
        var detail = chunks
            .map { $0.rawDetail ?? $0.detail }
            .filter { !$0.isEmpty }
            .joined()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if detail.isEmpty {
            detail = override?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        }
        return boundedRawResponseDetail(detail)
    }

    private static func summaryDetail(
        for chunks: [P031RuntimeTimelineEventPresentation],
        rawDetail: String
    ) -> String {
        let chunkCount = chunks.reduce(0) { total, chunk in
            total + (chunk.chunkCount ?? 1)
        }
        let byteCount = rawDetail.utf8.count
        return "Response complete · \(chunkCount) chunk\(chunkCount == 1 ? "" : "s") · \(byteCount) byte\(byteCount == 1 ? "" : "s") retained"
    }
}

enum P031RunDetailTab: String, CaseIterable, Identifiable {
    case overview
    case stages
    case artifacts
    case approvals
    case timeline
    case reports
    case system

    var id: String { rawValue }

    var title: String {
        switch self {
        case .overview:
            return "Overview"
        case .stages:
            return "Stages"
        case .artifacts:
            return "Artifacts"
        case .approvals:
            return "Approvals"
        case .timeline:
            return "Timeline"
        case .reports:
            return "Reports"
        case .system:
            return "System"
        }
    }
}

private enum P077CloseoutReadinessAnchor: Hashable {
    case card
}

private enum P077CloseoutReadinessFocus: Hashable {
    case compactSignal
    case diagnosticsTrigger
    case copyGeneration
    case primaryUnblock
    case secondaryBlocker(String)
    case copyFallback
    case recoveryLifecycle
    case copyRecoveryTemplate
    case recoveryCopyFeedback
    case backlinkRoute
    case modeExplainer
}

nonisolated final class P081ApprovalActionAttemptStore: @unchecked Sendable {
    private let defaults: UserDefaults
    private let storageKey: String
    private let makeID: @Sendable () -> String
    private let lock = NSLock()

    init(
        defaults: UserDefaults = .standard,
        storageKey: String = "chainworks.p081.approval-action-attempts.v1",
        makeID: @escaping @Sendable () -> String = { makeUUIDv7() }
    ) {
        self.defaults = defaults
        self.storageKey = storageKey
        self.makeID = makeID
    }

    func idempotencyKey(for approvalID: String, action: P072ApprovalDecisionAction) -> String {
        let attemptKey = Self.attemptStorageKey(approvalID: approvalID, action: action)
        lock.lock()
        defer { lock.unlock() }

        var attempts = loadLocked()
        if let existing = attempts[attemptKey] {
            return existing
        }

        let created = makeID()
        attempts[attemptKey] = created
        saveLocked(attempts)
        return created
    }

    func clear(approvalID: String, action: P072ApprovalDecisionAction) {
        let attemptKey = Self.attemptStorageKey(approvalID: approvalID, action: action)
        lock.lock()
        defer { lock.unlock() }

        var attempts = loadLocked()
        attempts.removeValue(forKey: attemptKey)
        saveLocked(attempts)
    }

    func clearAll() {
        lock.lock()
        defer { lock.unlock() }
        defaults.removeObject(forKey: storageKey)
    }

    private func loadLocked() -> [String: String] {
        guard let raw = defaults.dictionary(forKey: storageKey) else {
            return [:]
        }
        return raw.compactMapValues { $0 as? String }
    }

    private func saveLocked(_ attempts: [String: String]) {
        if attempts.isEmpty {
            defaults.removeObject(forKey: storageKey)
        } else {
            defaults.set(attempts, forKey: storageKey)
        }
    }

    private static func attemptStorageKey(
        approvalID: String,
        action: P072ApprovalDecisionAction
    ) -> String {
        let actionComponent: String
        switch action {
        case .approve:
            actionComponent = "approve"
        case .reject(let reason):
            actionComponent = "reject:\(escaped(reason))"
        }
        return "approval:\(escaped(approvalID))|action:\(actionComponent)"
    }

    private static func escaped(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? value
    }
}

@MainActor
final class P031ThinReadDashboardModel: ObservableObject {
    @Published private(set) var runsHome: P031RunsHomePresentation?
    @Published private(set) var runDetail: P031RunDetailPresentation?
    @Published private(set) var approvalInbox: P031ApprovalInboxPresentation?
    @Published private(set) var daemonLifecycle: P031DaemonLifecyclePresentation?
    @Published private(set) var schedulerHealth: SchedulerHealthReadback?
    @Published private(set) var isLoading = false
    @Published private(set) var isRestartingDaemon = false
    @Published private(set) var resolvingApprovalIDs: Set<String> = []
    @Published private(set) var approvalActionError: String?
    @Published private(set) var daemonRestartError: String?
    @Published private(set) var selectedRunID: String?
    @Published private(set) var runtimeTimelineEvents: [P031RuntimeTimelineEventPresentation] = []

    var totalPendingApprovalCount: Int {
        approvalInbox?.rows.count ?? 0
    }

    private let loadRunsHomeAction: @Sendable (P031FreshnessSnapshot, Bool) async -> P031RunsHomePresentation
    private let loadRunDetailAction: @Sendable (String, P031FreshnessSnapshot) async -> P031RunDetailPresentation
    private let loadArtifactPreviewAction: (String) async -> P031ArtifactViewerPresentation?
    private let resolveTimelineRawDetailAction: @Sendable (String) async -> P031TimelineRawDetailReadModel
    private let loadApprovalInboxAction: @Sendable (P031FreshnessSnapshot) async -> P031ApprovalInboxPresentation
    private let loadDaemonLifecycleAction: @Sendable (P031FreshnessSnapshot) async -> P031DaemonLifecyclePresentation
    private let loadSchedulerHealthAction: @Sendable () async -> SchedulerHealthReadback?
    private let subscribeRunStatusAction: @Sendable (String, P031FreshnessSnapshot) throws -> AsyncThrowingStream<P031RunStatusSubscriptionPresentation, Error>
    private let subscribeRuntimeTimelineAction: @Sendable (String) throws -> AsyncThrowingStream<P031RuntimeTimelineEventPresentation, Error>
    private let settleApprovalAction: @Sendable (String, P072ApprovalDecisionAction) async -> String?
    private let restartDaemonAction: @MainActor @Sendable () async -> String?
    private let bundledDaemonBuildSHAAction: @Sendable () -> String?

    private var didLoad = false
    private var runsFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runDetailFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var approvalFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var daemonFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runStatusSubscriptionTask: Task<Void, Never>?
    private var runtimeTimelineSubscriptionTask: Task<Void, Never>?
    private var runtimeTimelinePublishTask: Task<Void, Never>?
    private var subscribedRunID: String?
    private var subscribedRuntimeTimelineRunID: String?
    private var runtimeTimelineBuffer = P036RuntimeTimelineBuffer()
    private var runtimeTimelineLastPublish = Date.distantPast
    private let runtimeTimelineFlushInterval: TimeInterval

    var daemonSchemaMismatchMessage: String? {
        [
            runsHome?.errorDescription,
            runDetail?.errorDescription,
            approvalInbox?.errorDescription,
            daemonLifecycle?.errorDescription,
        ]
        .compactMap { $0 }
        .first(where: P031ReadErrorPresenter.isSchemaMismatchDescription)
    }

    var daemonBuildMismatchMessage: String? {
        guard let liveBuildSHA = daemonLifecycle?.buildSHA?.trimmingCharacters(in: .whitespacesAndNewlines),
              !liveBuildSHA.isEmpty,
              let bundledBuildSHA = bundledDaemonBuildSHAAction()?.trimmingCharacters(in: .whitespacesAndNewlines),
              !bundledBuildSHA.isEmpty,
              !Self.buildSHA(liveBuildSHA, matches: bundledBuildSHA)
        else {
            return nil
        }
        return "Live daemon build \(liveBuildSHA) does not match bundled daemon build \(bundledBuildSHA). Restart daemon to load the bundled control plane."
    }

    init<Store: P031WorkflowReadStore>(
        coordinator: P031ThinWorkflowScreenCoordinator<Store>,
        settleApprovalAction: @escaping @Sendable (String, P072ApprovalDecisionAction) async -> String? = { _, _ in
            "Approval write path is unavailable in this build."
        },
        restartDaemonAction: @escaping @MainActor @Sendable () async -> String? = {
            await P031ThinReadDashboardModel.restartPackagedDaemon()
        },
        bundledDaemonBuildSHAAction: @escaping @Sendable () -> String? = {
            P031ThinReadDashboardModel.bundledDaemonBuildSHA()
        },
        loadSchedulerHealthAction: @escaping @Sendable () async -> SchedulerHealthReadback? = { nil },
        runtimeTimelineFlushInterval: TimeInterval = 2.0
    ) {
        self.settleApprovalAction = settleApprovalAction
        self.restartDaemonAction = restartDaemonAction
        self.bundledDaemonBuildSHAAction = bundledDaemonBuildSHAAction
        self.loadSchedulerHealthAction = loadSchedulerHealthAction
        self.runtimeTimelineFlushInterval = runtimeTimelineFlushInterval
        loadRunsHomeAction = { currentFreshness, showFirstRunOrientation in
            await coordinator.loadRunsHome(
                currentFreshness: currentFreshness,
                showFirstRunOrientation: showFirstRunOrientation
            )
        }
        loadRunDetailAction = { runID, currentFreshness in
            await coordinator.loadRunDetail(runID: runID, currentFreshness: currentFreshness)
        }
        loadArtifactPreviewAction = { artifactID in
            await coordinator.loadArtifactPreview(artifactID: artifactID)
        }
        resolveTimelineRawDetailAction = { handle in
            await coordinator.resolveTimelineRawDetail(handle: handle)
        }
        loadApprovalInboxAction = { currentFreshness in
            await coordinator.loadApprovalInbox(currentFreshness: currentFreshness)
        }
        loadDaemonLifecycleAction = { currentFreshness in
            await coordinator.loadDaemonLifecycle(currentFreshness: currentFreshness)
        }
        let subscriptionCoordinator = P031ThinWorkflowSubscriptionCoordinator(store: coordinator.store)
        subscribeRunStatusAction = { runID, currentFreshness in
            try subscriptionCoordinator.runStatusPresentations(
                runID: runID,
                currentFreshness: currentFreshness
            )
        }
        subscribeRuntimeTimelineAction = { runID in
            try subscriptionCoordinator.runtimeTimelinePresentations(runID: runID)
        }
    }

    deinit {
        runStatusSubscriptionTask?.cancel()
        runtimeTimelineSubscriptionTask?.cancel()
        runtimeTimelinePublishTask?.cancel()
    }

    static func bootstrap(
        approvalActionAttemptStore: P081ApprovalActionAttemptStore = P081ApprovalActionAttemptStore()
    ) -> P031ThinReadDashboardModel {
        let endpoint = DaemonClientEndpoint.operatorDefault()
        let guideResource = P031OperatorWritePathGuideBootstrap.load()
        let store = P031GraphQLWorkflowReadStore(
            readTransport: P031URLSessionGraphQLReadTransport(endpoint: endpoint),
            subscriptionTransport: P031URLSessionGraphQLSubscriptionTransport(endpoint: endpoint)
        )
        let approvalMutationClient = P072ApprovalMutationClient(
            transport: P031URLSessionGraphQLReadTransport(endpoint: endpoint)
        )
        let coordinator = P031ThinWorkflowScreenCoordinator(
            store: store,
            writePathGuideData: guideResource.data
        )
        let lifecycleClient = DaemonLifecycleClient(endpoint: endpoint)
        return P031ThinReadDashboardModel(
            coordinator: coordinator,
            settleApprovalAction: { approvalID, action in
                do {
                    let idempotencyKey = approvalActionAttemptStore.idempotencyKey(
                        for: approvalID,
                        action: action
                    )
                    switch action {
                    case .approve:
                        _ = try await approvalMutationClient.approve(
                            approvalID: approvalID,
                            idempotencyKey: idempotencyKey
                        )
                    case .reject(let reason):
                        _ = try await approvalMutationClient.reject(
                            approvalID: approvalID,
                            reason: reason,
                            idempotencyKey: idempotencyKey
                        )
                    }
                    approvalActionAttemptStore.clear(approvalID: approvalID, action: action)
                    return nil
                } catch {
                    // On error the key is retained so the next retry reuses the same key.
                    return P031ReadErrorPresenter.description(for: error)
                }
            },
            loadSchedulerHealthAction: {
                try? await lifecycleClient.schedulerReadback()
            }
        )
    }

#if DEBUG
    static func previewLoaded() -> P031ThinReadDashboardModel {
        previewLoaded(includeCloseoutReadiness: false)
    }

    static func previewLoadedWithCloseoutReadiness() -> P031ThinReadDashboardModel {
        previewLoaded(includeCloseoutReadiness: true)
    }

    private static func previewLoaded(includeCloseoutReadiness: Bool) -> P031ThinReadDashboardModel {
        let freshness = P031FreshnessSnapshot(state: .live, lastCheckedAt: Date())
        let runID = "preview-run-proposal-review"
        let artifacts = previewArtifacts(freshness: freshness)
        let detail = previewRunDetail(
            runID: runID,
            freshness: freshness,
            artifacts: artifacts,
            includeCloseoutReadiness: includeCloseoutReadiness
        )
        let closeoutSignal = detail.closeoutReadiness?.compactSignalLabel
        let runsHome = P031RunsHomePresentation(
            orientation: nil,
            rows: [
                P031RunsHomeRowPresentation(
                    runID: runID,
                    title: "Proposal review run",
                    workflowLabel: "chainworks_proposal_review",
                    statusLabel: "Running",
                    progressLabel: "13 stages, 48 artifacts",
                    pendingApprovalsLabel: nil,
                    closeoutReadinessSignalLabel: closeoutSignal,
                    implementationCompletionSignalLabel: detail.implementationCompletion?.compactSignalLabel,
                    sideEffectSignalLabel: detail.sideEffectReadback?.compactSignalLabel,
                    freshnessState: .live,
                    accessibilityLabel: "Proposal review run, running",
                    rawStatus: "running",
                    failedStages: 0,
                    pendingApprovals: 0
                ),
                P031RunsHomeRowPresentation(
                    runID: "preview-run-implementation",
                    title: "Implementation closeout",
                    workflowLabel: "chainworks_implementation",
                    statusLabel: "Completed",
                    progressLabel: "9 stages, 31 artifacts",
                    pendingApprovalsLabel: nil,
                    closeoutReadinessSignalLabel: nil,
                    freshnessState: .live,
                    accessibilityLabel: "Implementation closeout, completed",
                    rawStatus: "completed",
                    failedStages: 0,
                    pendingApprovals: 0
                ),
            ],
            freshness: freshness,
            refreshFeedbackText: "Live projection",
            emptyStateTitle: nil,
            errorDescription: nil
        )
        let approvals = P031ApprovalInboxPresentation(
            rows: [],
            freshness: freshness,
            refreshFeedbackText: "No pending approvals",
            emptyStateTitle: "No approvals",
            errorDescription: nil
        )
        let daemon = P031DaemonLifecyclePresentation(
            state: .ready,
            buildSHA: "preview",
            pid: 1234,
            uptimeSeconds: 3600,
            title: "Control plane daemon",
            detailLabel: "Running on local GraphQL endpoint",
            badgeLabels: ["Running", "Live"],
            copyItems: [],
            freshness: freshness,
            refreshFeedbackText: "Live projection",
            errorDescription: nil
        )
        return P031ThinReadDashboardModel(
            runsHome: runsHome,
            runDetail: detail,
            approvalInbox: approvals,
            daemonLifecycle: daemon,
            selectedRunID: runID
        )
    }

    private init(
        runsHome: P031RunsHomePresentation,
        runDetail: P031RunDetailPresentation,
        approvalInbox: P031ApprovalInboxPresentation,
        daemonLifecycle: P031DaemonLifecyclePresentation,
        selectedRunID: String
    ) {
        loadRunsHomeAction = { _, _ in runsHome }
        loadRunDetailAction = { _, _ in runDetail }
        loadArtifactPreviewAction = { artifactID in
            runDetail.artifactViewerRows.first { $0.artifactID == artifactID }
        }
        resolveTimelineRawDetailAction = { _ in
            P031TimelineRawDetailReadModel(
                status: .missing,
                rawDetail: nil,
                rawDetailBytes: nil,
                rawDetailDigest: nil,
                errorReason: .handleNotFound
            )
        }
        loadApprovalInboxAction = { _ in approvalInbox }
        loadDaemonLifecycleAction = { _ in daemonLifecycle }
        subscribeRunStatusAction = { _, _ in
            AsyncThrowingStream { continuation in
                continuation.finish()
            }
        }
        subscribeRuntimeTimelineAction = { _ in
            AsyncThrowingStream { continuation in
                continuation.finish()
            }
        }
        settleApprovalAction = { _, _ in nil }
        restartDaemonAction = { nil }
        bundledDaemonBuildSHAAction = { "preview" }
        loadSchedulerHealthAction = { nil }
        runtimeTimelineFlushInterval = 2.0

        self.runsHome = runsHome
        self.runDetail = runDetail
        self.approvalInbox = approvalInbox
        self.daemonLifecycle = daemonLifecycle
        self.selectedRunID = selectedRunID
        runsFreshness = runsHome.freshness
        runDetailFreshness = runDetail.freshness
        approvalFreshness = approvalInbox.freshness
        daemonFreshness = daemonLifecycle.freshness
        didLoad = true
    }

    private static func previewRunDetail(
        runID: String,
        freshness: P031FreshnessSnapshot,
        artifacts: [P031ArtifactViewerPresentation],
        includeCloseoutReadiness: Bool = false
    ) -> P031RunDetailPresentation {
        let transitions = [
            P031StageTransitionPresentation(
                stageExecutionID: "stage-iteration-11-attempt-1",
                stageTitle: "Proposal reviewed",
                statusText: "Skipped",
                attemptText: "Iteration 11, attempt 1",
                startedLabel: "Started: 2026-05-09 09:12",
                completedLabel: "Completed: 2026-05-09 09:12",
                durationLabel: "Duration: 8s",
                connectorState: .pending,
                evidenceLabels: ["Artifacts", "Skipped"],
                accessibilityLabel: "Proposal reviewed, iteration 11 attempt 1, skipped"
            ),
            P031StageTransitionPresentation(
                stageExecutionID: "stage-iteration-11-attempt-6",
                stageTitle: "Proposal reviewed",
                statusText: "Completed",
                attemptText: "Iteration 11, attempt 6",
                startedLabel: "Started: 2026-05-09 09:30",
                completedLabel: "Completed: 2026-05-09 09:34",
                durationLabel: "Duration: 4m 18s",
                connectorState: .completed,
                evidenceLabels: ["Artifacts", "Validation", "Completed"],
                accessibilityLabel: "Proposal reviewed, iteration 11 attempt 6, completed"
            ),
            P031StageTransitionPresentation(
                stageExecutionID: "stage-iteration-13-attempt-1",
                stageTitle: "Proposal reviewed",
                statusText: "Running",
                attemptText: "Iteration 13, attempt 1",
                startedLabel: "Started: 2026-05-09 10:02",
                completedLabel: nil,
                durationLabel: "Duration: 2m 41s",
                connectorState: .running,
                evidenceLabels: ["Artifacts"],
                accessibilityLabel: "Proposal reviewed, iteration 13 attempt 1, running"
            ),
        ]
        return P031RunDetailPresentation(
            title: "Proposal review run",
            workflowLabel: "chainworks_proposal_review",
            statusLabel: "Running",
            progressLabel: "3 visible stages, 48 artifacts",
            pendingApprovalsLabel: nil,
            rolloutDecisionSummary: nil,
            ideaContext: P031IdeaContextPresentation(
                id: "idea-preview",
                title: "Improve artifact navigation",
                statusLabel: "In review",
                projectKey: "P031",
                body: "Make repeated artifacts understandable across iterations and attempts.",
                createdAt: "2026-04-30",
                archivedAt: nil,
                accessibilityLabel: "Improve artifact navigation"
            ),
            stageTransitions: transitions,
            approvalRows: [],
            artifactRows: [],
            artifactViewerRows: artifacts,
            reportRows: [],
            catalogContext: P031CatalogContextPresentation(
                workflowID: "chainworks_proposal_review",
                workflowTitle: "Proposal review",
                workflowSnapshotHash: "preview-workflow",
                catalogSnapshotHash: "preview-catalog",
                statusText: "Catalog snapshot available",
                accessibilityLabel: "Proposal review catalog snapshot available"
            ),
            closeoutReadiness: includeCloseoutReadiness
                ? previewCloseoutReadiness(runID: runID)
                : nil,
            implementationCompletion: previewImplementationCompletion(),
            freshness: freshness,
            refreshFeedbackText: "Live projection",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "running",
            failedStages: 0
        )
    }

    private static func previewCloseoutReadiness(runID: String) -> P077CloseoutReadinessPresentation? {
        let json = """
        {
          "run_id": "\(runID)",
          "stage_id": "state_9_implementation_reviewed",
          "readiness_status": "not_ready",
          "readiness_decision": "return_to_code_refine",
          "readiness_generation_id": "abcdef1234567890",
          "readiness_mode": "enforcement",
          "gate_status": "failed",
          "gate_generation_id": "gateabcdef123456",
          "audit_status": "not_ready",
          "diagnostic_reason": "proposal-077 gate failed",
          "primary_unblock": "Fix implementation blockers",
          "code_blocker_count": 2,
          "handoff_count": 0,
          "risk_settlement_required": false,
          "accepted_risk_count": 0,
          "fingerprint_hash": "sha256:fixture",
          "summary": "Fix implementation blockers",
          "synthesized_at": "2026-05-06T09:55:45Z",
          "is_applicable": true
        }
        """
        guard let data = json.data(using: .utf8),
              let summary = try? JSONDecoder().decode(P077CloseoutReadinessSummaryReadModel.self, from: data)
        else {
            return nil
        }
        return P077CloseoutReadinessPresenter.presentation(for: summary)
    }

    private static func previewImplementationCompletion() -> P088ImplementationCompletionPresentation {
        P088ImplementationCompletionPresenter.presentation(
            for: P088ImplementationCompletionReadModel(
                status: .known(value: "partial_evidence"),
                failureClass: "work_completed_missing_current_attempt_outputs",
                workChangeKind: "current_attempt_diff",
                activationSource: "declared_output_settlement_failed",
                ingestionBoundaryFailure: .known(value: "chainworks_output_not_extracted"),
                completionTurnAttempted: true,
                completionTurnResult: .known(value: "failed_missing_outputs"),
                terminalResponseStatus: "completed",
                completionTextCaptures: [],
                freshRequiredOutputCount: 1,
                staleRequiredOutputCount: 1,
                missingRequiredOutputCount: 2,
                controlPlaneOutputCount: 1,
                receiptArtifactPath: ".chainworks/p088/receipt.json",
                failedStageEvidencePath: ".chainworks/p088/failed-stage.json",
                nextOperatorAction: .known(value: "fix_chainworks_output_extraction")
            )
        )
    }

    private static func previewArtifacts(
        freshness: P031FreshnessSnapshot
    ) -> [P031ArtifactViewerPresentation] {
        [
            previewArtifact(
                id: "artifact-review-summary-11-1",
                stageID: "state_4_proposal_reviewed",
                stageExecutionID: "stage-iteration-11-attempt-1",
                iteration: 11,
                attempt: 1,
                title: "proposal_review_summary",
                contractID: "proposal_review_summary_v2",
                content: "# Review summary\n\nSkipped attempt retained its artifact set for audit history.",
                freshness: freshness
            ),
            previewArtifact(
                id: "artifact-review-summary-11-6",
                stageID: "state_4_proposal_reviewed",
                stageExecutionID: "stage-iteration-11-attempt-6",
                iteration: 11,
                attempt: 6,
                title: "proposal_review_summary",
                contractID: "proposal_review_summary_v2",
                content: "# Review summary\n\nThe latest completed attempt includes validation and closeout notes.",
                freshness: freshness
            ),
            previewArtifact(
                id: "artifact-review-corpus-11-6",
                stageID: "state_4_proposal_reviewed",
                stageExecutionID: "stage-iteration-11-attempt-6",
                iteration: 11,
                attempt: 6,
                title: "review_corpus_bundle",
                contractID: "review_corpus_bundle_v1",
                content: "{\n  \"documents\": 12,\n  \"latestReport\": \"proposal_review_summary\"\n}",
                freshness: freshness
            ),
            previewArtifact(
                id: "artifact-review-summary-13-1",
                stageID: "state_4_proposal_reviewed",
                stageExecutionID: "stage-iteration-13-attempt-1",
                iteration: 13,
                attempt: 1,
                title: "proposal_review_summary",
                contractID: "proposal_review_summary_v2",
                content: "# Running review\n\nThis attempt is still collecting artifacts.",
                freshness: freshness
            ),
        ]
    }

    private static func previewArtifact(
        id: String,
        stageID: String,
        stageExecutionID: String,
        iteration: Int,
        attempt: Int,
        title: String,
        contractID: String,
        content: String,
        freshness: P031FreshnessSnapshot
    ) -> P031ArtifactViewerPresentation {
        P031ArtifactViewerPresentation(
            artifactID: id,
            stageID: stageID,
            stageExecutionID: stageExecutionID,
            stageLabel: "Proposal reviewed",
            iteration: iteration,
            attemptNumber: attempt,
            agentID: "lead_orchestrator",
            contractID: contractID,
            format: title == "review_corpus_bundle" ? "json" : "markdown",
            title: title,
            subtitle: "\(contractID) / \(title == "review_corpus_bundle" ? "json" : "markdown") / lead_orchestrator",
            renderMode: title == "review_corpus_bundle" ? .json : .markdown,
            payloadState: .available,
            preparedPreview: ArtifactPreviewPolicy.prepare(
                content: content,
                intent: title == "review_corpus_bundle" ? .jsonTree(rescuedFrom: nil) : .markdownDocument
            ),
            unavailableReason: nil,
            freshnessState: freshness.state,
            accessibilityLabel: "\(title), iteration \(iteration), attempt \(attempt)"
        )
    }
#endif

    func loadIfNeeded() async {
        guard !didLoad else { return }
        didLoad = true
        await refreshAll()
    }

    func refreshAll() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }

        async let runsTask = loadRunsHomeAction(runsFreshness, false)
        async let approvalsTask = loadApprovalInboxAction(approvalFreshness)
        async let daemonTask = loadDaemonLifecycleAction(daemonFreshness)
        async let schedulerTask = loadSchedulerHealthAction()

        let runsPresentation = await runsTask
        let approvalsPresentation = await approvalsTask
        let daemonPresentation = await daemonTask
        let schedulerResult = await schedulerTask

        runsFreshness = runsPresentation.freshness
        approvalFreshness = approvalsPresentation.freshness
        daemonFreshness = daemonPresentation.freshness

        runsHome = runsPresentation
        approvalInbox = approvalsPresentation
        daemonLifecycle = daemonPresentation
        schedulerHealth = schedulerResult

        let availableRunIDs = runsPresentation.rows.map { $0.runID }
        if let selectedRunID, availableRunIDs.contains(selectedRunID) {
            await loadRunDetail(for: selectedRunID)
            startLiveSubscriptions(for: selectedRunID)
        } else if let firstRunID = availableRunIDs.first {
            selectedRunID = firstRunID
            await loadRunDetail(for: firstRunID)
            startLiveSubscriptions(for: firstRunID)
        } else {
            selectedRunID = nil
            runDetail = nil
            stopLiveSubscriptions()
        }
    }

    func selectRun(_ runID: String) {
        guard selectedRunID != runID else { return }
        P036UICounters.shared.recordOperatorTaskAttempt(
            taskID: "runs.select_run",
            result: "started",
            blockedReason: nil
        )
        selectedRunID = runID
        resetRuntimeTimelineBuffer()
        startLiveSubscriptions(for: runID)
        Task { await loadRunDetail(for: runID) }
    }

    func loadArtifactPreview(artifactID: String) async -> P031ArtifactViewerPresentation? {
        let preview = await loadArtifactPreviewAction(artifactID)
        P036UICounters.shared.recordOperatorTaskAttempt(
            taskID: "runs.load_artifact_preview",
            result: preview == nil ? "blocked" : "succeeded",
            blockedReason: preview == nil ? "payload_unavailable" : nil
        )
        return preview
    }

    func resolveTimelineRawDetail(handle: String) async -> P031TimelineRawDetailReadModel {
        await resolveTimelineRawDetailAction(handle)
    }

    func isResolvingApproval(_ approvalID: String) -> Bool {
        resolvingApprovalIDs.contains(approvalID)
    }

    func settleApproval(_ approvalID: String, action: P072ApprovalDecisionAction) async {
        guard !resolvingApprovalIDs.contains(approvalID) else {
            P036UICounters.shared.recordOperatorTaskAttempt(
                taskID: "runs.settle_approval",
                result: "blocked",
                blockedReason: "already_resolving"
            )
            return
        }
        resolvingApprovalIDs.insert(approvalID)
        approvalActionError = nil
        defer { resolvingApprovalIDs.remove(approvalID) }

        if let error = await settleApprovalAction(approvalID, action) {
            approvalActionError = error
            P036UICounters.shared.recordOperatorTaskAttempt(
                taskID: "runs.settle_approval",
                result: "blocked",
                blockedReason: "write_failed"
            )
            return
        }
        P036UICounters.shared.recordOperatorTaskAttempt(
            taskID: "runs.settle_approval",
            result: "succeeded",
            blockedReason: nil
        )
        await refreshAll()
    }

    func restartDaemonForSchemaMismatch() async {
        await restartDaemonForUpdateRequired()
    }

    func restartDaemonForUpdateRequired() async {
        guard !isRestartingDaemon else {
            P036UICounters.shared.recordOperatorTaskAttempt(
                taskID: "system.restart_daemon",
                result: "blocked",
                blockedReason: "already_restarting"
            )
            return
        }
        isRestartingDaemon = true
        daemonRestartError = nil
        defer { isRestartingDaemon = false }

        if let error = await restartDaemonAction() {
            daemonRestartError = error
            P036UICounters.shared.recordOperatorTaskAttempt(
                taskID: "system.restart_daemon",
                result: "blocked",
                blockedReason: "restart_failed"
            )
            return
        }
        P036UICounters.shared.recordOperatorTaskAttempt(
            taskID: "system.restart_daemon",
            result: "succeeded",
            blockedReason: nil
        )
        await refreshAll()
    }

    func refreshDaemonLifecycle() async {
        let presentation = await loadDaemonLifecycleAction(daemonFreshness)
        daemonFreshness = presentation.freshness
        daemonLifecycle = presentation

        let schedulerResult = await loadSchedulerHealthAction()
        schedulerHealth = schedulerResult
    }

    private func loadRunDetail(for runID: String) async {
        let presentation = await loadRunDetailAction(runID, runDetailFreshness)
        runDetailFreshness = presentation.freshness
        runDetail = presentation
    }

    private func startLiveSubscriptions(for runID: String) {
        startRunStatusSubscription(for: runID)
        startRuntimeTimelineSubscription(for: runID)
    }

    private func startRunStatusSubscription(for runID: String) {
        guard subscribedRunID != runID else { return }
        runStatusSubscriptionTask?.cancel()
        subscribedRunID = runID
        let freshness = runDetailFreshness
        runStatusSubscriptionTask = Task { [weak self] in
            guard let self else { return }
            do {
                let stream = try self.subscribeRunStatusAction(runID, freshness)
                for try await event in stream {
                    try Task.checkCancellation()
                    await self.refreshSelectedRunAfterSubscriptionEvent(runID: event.runID)
                }
            } catch is CancellationError {
                return
            } catch {
                await self.refreshSelectedRunAfterSubscriptionEvent(runID: runID)
            }
        }
    }

    private func startRuntimeTimelineSubscription(for runID: String) {
        guard subscribedRuntimeTimelineRunID != runID else { return }
        runtimeTimelineSubscriptionTask?.cancel()
        subscribedRuntimeTimelineRunID = runID
        resetRuntimeTimelineBuffer()
        runtimeTimelineSubscriptionTask = Task { [weak self] in
            guard let self else { return }
            do {
                let stream = try self.subscribeRuntimeTimelineAction(runID)
                for try await event in stream {
                    try Task.checkCancellation()
                    self.recordRuntimeTimelineEvent(event)
                }
            } catch is CancellationError {
                return
            } catch {
                return
            }
        }
    }

    private func stopLiveSubscriptions() {
        stopRunStatusSubscription()
        runtimeTimelineSubscriptionTask?.cancel()
        runtimeTimelineSubscriptionTask = nil
        runtimeTimelinePublishTask?.cancel()
        runtimeTimelinePublishTask = nil
        subscribedRuntimeTimelineRunID = nil
        resetRuntimeTimelineBuffer()
    }

    private func stopRunStatusSubscription() {
        runStatusSubscriptionTask?.cancel()
        runStatusSubscriptionTask = nil
        subscribedRunID = nil
    }

    private func recordRuntimeTimelineEvent(_ event: P031RuntimeTimelineEventPresentation) {
        let forcePublish = Self.isRuntimeTimelineTerminalEvent(event)
        runtimeTimelineBuffer.record(event, selectedRunID: selectedRunID)
        scheduleRuntimeTimelinePublish(force: forcePublish || runtimeTimelineEvents.isEmpty)
    }

    private func scheduleRuntimeTimelinePublish(force: Bool) {
        guard !runtimeTimelineBuffer.events.isEmpty else { return }
        let elapsed = Date().timeIntervalSince(runtimeTimelineLastPublish)
        if force || runtimeTimelineFlushInterval <= 0 || elapsed >= runtimeTimelineFlushInterval {
            publishRuntimeTimelineEvents()
            return
        }
        guard runtimeTimelinePublishTask == nil else { return }
        let delay = max(0, runtimeTimelineFlushInterval - elapsed)
        runtimeTimelinePublishTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            await MainActor.run {
                self?.publishRuntimeTimelineEvents()
            }
        }
    }

    private func publishRuntimeTimelineEvents() {
        runtimeTimelinePublishTask?.cancel()
        runtimeTimelinePublishTask = nil
        runtimeTimelineEvents = runtimeTimelineBuffer.events
        runtimeTimelineLastPublish = Date()
        P036UICounters.shared.recordTimelineBatchFlush(
            entryCount: runtimeTimelineEvents.count,
            reduceMotion: false
        )
    }

    private func resetRuntimeTimelineBuffer() {
        runtimeTimelinePublishTask?.cancel()
        runtimeTimelinePublishTask = nil
        runtimeTimelineBuffer.reset()
        runtimeTimelineEvents = []
        runtimeTimelineLastPublish = Date.distantPast
    }

    private static func isRuntimeTimelineTerminalEvent(
        _ event: P031RuntimeTimelineEventPresentation
    ) -> Bool {
        event.surfaceLabel == "final_response"
            || event.eventKind == "session_completed"
            || event.eventKind == "session_failed"
    }

    private func refreshSelectedRunAfterSubscriptionEvent(runID: String) async {
        guard selectedRunID == runID else { return }

        async let runsTask = loadRunsHomeAction(runsFreshness, false)
        async let approvalsTask = loadApprovalInboxAction(approvalFreshness)
        async let detailTask = loadRunDetailAction(runID, runDetailFreshness)
        async let schedulerTask = loadSchedulerHealthAction()

        let runsPresentation = await runsTask
        let approvalsPresentation = await approvalsTask
        let detailPresentation = await detailTask
        let schedulerResult = await schedulerTask

        guard selectedRunID == runID else { return }
        runsFreshness = runsPresentation.freshness
        approvalFreshness = approvalsPresentation.freshness
        runDetailFreshness = detailPresentation.freshness
        runsHome = runsPresentation
        approvalInbox = approvalsPresentation
        runDetail = detailPresentation
        schedulerHealth = schedulerResult
    }

    private static func restartPackagedDaemon() async -> String? {
#if os(macOS)
        do {
            try await Chainworks_ForgeApp.restartPackagedDaemonAgent()
            return nil
        } catch {
            return error.localizedDescription
        }
#else
        return "Daemon restart is only available on macOS."
#endif
    }

    nonisolated static func bundledDaemonBuildSHA(bundle: Bundle = .main) -> String? {
        guard let url = bundle.url(forResource: "bundled-daemon-build-sha", withExtension: "txt"),
              let data = try? Data(contentsOf: url),
              let raw = String(data: data, encoding: .utf8)
        else {
            return nil
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    nonisolated private static func buildSHA(
        _ liveBuildSHA: String,
        matches bundledBuildSHA: String
    ) -> Bool {
        liveBuildSHA == bundledBuildSHA
            || liveBuildSHA.hasPrefix("\(bundledBuildSHA)-")
            || bundledBuildSHA.hasPrefix("\(liveBuildSHA)-")
    }
}

private struct P031DaemonUpdateRequiredCard: View {
    let title: String
    let message: String
    let restartError: String?
    let isRestarting: Bool
    let onRestart: () -> Void

    var body: some View {
        P031CalloutCard(
            title: title,
            bodyText: message,
            accentColor: .orange
        ) {
            HStack(spacing: 10) {
                Button {
                    onRestart()
                } label: {
                    Label(
                        isRestarting ? "Restarting daemon" : "Restart daemon",
                        systemImage: "arrow.triangle.2.circlepath"
                    )
                }
                .disabled(isRestarting)
                .controlSize(.small)
                .accessibilityIdentifier("p031-daemon-schema-mismatch-restart")

                if let restartError {
                    Text(restartError)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
        }
    }
}

struct P031OperatorWritePathGuideBootstrapResource {
    let data: Data?
    let url: URL?
}

enum P031OperatorWritePathGuideBootstrap {
    private static let guideResourceName = "p031-operator-write-path-guide"
    private static let guideResourceExtension = "json"
    private static let guideRepoRelativePath = "docs/reference/p031-operator-write-path-guide.json"

    static func load(
        currentDirectoryPath: String = FileManager.default.currentDirectoryPath,
        bundledURL: URL? = Bundle.main.url(
            forResource: guideResourceName,
            withExtension: guideResourceExtension
        ),
        sourceFilePath: String = #filePath
    ) -> P031OperatorWritePathGuideBootstrapResource {
        let url = AppConfiguration.preferredExampleURL(
            repoRelativePath: guideRepoRelativePath,
            bundledURL: bundledURL,
            currentDirectoryPath: currentDirectoryPath,
            sourceFilePath: sourceFilePath
        )
        let data = url.flatMap { try? Data(contentsOf: $0) }
        return P031OperatorWritePathGuideBootstrapResource(data: data, url: url)
    }
}

private struct P031SectionHeader: View {
    let title: String
    let subtitle: String
    let freshness: P031FreshnessSnapshot

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.headline)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            P031FreshnessBadge(snapshot: freshness)
        }
    }
}

private struct P031CalloutCard<Footer: View>: View {
    let title: String
    let bodyText: String
    let accentColor: Color
    @ViewBuilder var footer: Footer

    init(
        title: String,
        bodyText: String,
        accentColor: Color,
        @ViewBuilder footer: () -> Footer = { EmptyView() }
    ) {
        self.title = title
        self.bodyText = bodyText
        self.accentColor = accentColor
        self.footer = footer()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.headline)
            Text(bodyText)
                .font(.callout)
                .foregroundStyle(.secondary)
            footer
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 14))
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(accentColor.opacity(0.2), lineWidth: 1)
        )
    }
}

private struct P031RunsHomeRowCard: View {
    let row: P031RunsHomeRowPresentation
    let isSelected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(row.title)
                    .font(.headline)
                    .foregroundStyle(.primary)
                Spacer()
                P031FreshnessBadge(state: row.freshnessState)
            }
            if let workflowLabel = row.workflowLabel {
                Text(workflowLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            Text(row.statusLabel)
                .font(.subheadline.weight(.medium))
            if let closeoutReadinessSignalLabel = row.closeoutReadinessSignalLabel {
                Label(closeoutReadinessSignalLabel, systemImage: "checkmark.seal")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("p077-closeout-readiness-sidebar-signal")
            }
            if let implementationCompletionSignalLabel = row.implementationCompletionSignalLabel {
                Label(implementationCompletionSignalLabel, systemImage: "wrench.and.screwdriver")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("p088-implementation-completion-sidebar-signal")
            }
            if let sideEffectSignalLabel = row.sideEffectSignalLabel {
                Label(sideEffectSignalLabel, systemImage: "externaldrive.badge.exclamationmark")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("p078-side-effect-sidebar-signal")
            }
            if let progressLabel = row.progressLabel {
                Text(progressLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            if let pendingApprovalsLabel = row.pendingApprovalsLabel {
                Label(pendingApprovalsLabel, systemImage: "checklist")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .fill(isSelected ? Color.accentColor.opacity(0.16) : Color(nsColor: .controlBackgroundColor))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(isSelected ? Color.accentColor.opacity(0.45) : Color.clear, lineWidth: 1)
        )
        .accessibilityLabel(row.accessibilityLabel)
        .accessibilityIdentifier("runs-home-run-row")
    }
}

private struct TimelineEntryRow: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let entry: RunsWorkbenchPresentationModel.TimelineEntry
    let displayOrder: Int
    let isExpanded: Bool
    let resolveTimelineRawDetail: (String) async -> P031TimelineRawDetailReadModel
    let formatterCache: P093TimelineFormatterCache
    let onToggleExpanded: () -> Void

    @State private var isHovering = false
    @State private var resolvedFullRawDetail: String?
    @State private var rawDetailResolutionStatus: P031TimelineRawDetailStatus?
    @State private var rawDetailResolutionErrorReason: P031TimelineRawDetailErrorReason?
    @State private var isResolvingRawDetail = false
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Button {
                    isFocused = true
                    onToggleExpanded()
                } label: {
                    Image(systemName: isExpanded ? "chevron.down.circle.fill" : "chevron.right.circle")
                        .imageScale(.small)
                }
                .buttonStyle(.plain)
                .foregroundStyle(tint)
                .accessibilityLabel(isExpanded ? "Collapse Timeline event" : "Expand Timeline event")
                .accessibilityIdentifier("p093-timeline-toggle")

                Image(systemName: iconName)
                    .foregroundStyle(tint)
                    .accessibilityHidden(true)

                Text(entry.title)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(ForgeColor.Text.primary)
                    .lineLimit(1)

                if entry.isStreaming {
                    StatusCapsule(
                        text: "Streaming",
                        color: ForgeStatusColor.running,
                        icon: "waveform",
                        size: .small
                    )
                    .accessibilityLabel("Timeline status Streaming")
                    .accessibilityIdentifier("p093-timeline-status-badge")
                } else if entry.isTerminal {
                    StatusCapsule(
                        text: "Complete",
                        color: ForgeStatusColor.neutral,
                        icon: "checkmark.circle",
                        size: .small
                    )
                    .accessibilityLabel("Timeline status Complete")
                    .accessibilityIdentifier("p093-timeline-status-badge")
                }

                Spacer(minLength: 8)

                Text(entry.surfaceLabel)
                    .font(.caption2)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .lineLimit(1)
            }

            if isExpanded {
                expandedControls
                P093FormattedTimelineDetail(
                    result: formatterCache.render(
                        event: entry,
                        detail: expandedDetail,
                        detailDigest: entry.detailDigest,
                        detailCharCount: entry.detailCharCount,
                        chunkCount: entry.chunkCount
                    )
                )
                    .frame(
                        minHeight: P093TimelineFormattedResult.expandedMinimumHeight(for: expandedDetail),
                        maxHeight: 420,
                        alignment: .top
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .transaction { transaction in
                        transaction.animation = nil
                    }
                    .accessibilityIdentifier("p093-timeline-formatted-detail")
            } else if !entry.detail.isEmpty {
                Text(collapsedDetail)
                    .font(.caption)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .lineLimit(3)
                    .fixedSize(horizontal: false, vertical: false)
                    .frame(maxHeight: 64, alignment: .top)
                    .clipped()
            }

            HStack(spacing: 8) {
                if let providerID = entry.providerID, !providerID.isEmpty {
                    Text(providerID)
                        .accessibilityLabel("Provider \(providerID)")
                        .accessibilityIdentifier("p093-timeline-provider-badge")
                }
                if let agentID = entry.agentID, !agentID.isEmpty {
                    Text(agentID)
                        .accessibilityLabel("Agent \(agentID)")
                        .accessibilityIdentifier("p093-timeline-agent-badge")
                }
                Button {
                    copyToPasteboard(entry.id)
                } label: {
                    Label(shortID(entry.id), systemImage: "doc.on.doc")
                        .labelStyle(.titleAndIcon)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Copy Timeline event ID")
                .accessibilityIdentifier("p093-timeline-copy-id")
                .help("Copy Timeline event ID")

                if isHovering || isFocused {
                    metadataText
                }
            }
            .font(.caption2)
            .foregroundStyle(ForgeColor.Text.tertiary)
            .lineLimit(1)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onTapGesture {
            onToggleExpanded()
        }
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(tint.opacity(isExpanded ? 0.46 : 0.20), lineWidth: isExpanded ? 1.5 : 1)
        )
        .focusable(true)
        .focused($isFocused)
        .onKeyPress(.space) {
            onToggleExpanded()
            return .handled
        }
        .onKeyPress(.return) {
            onToggleExpanded()
            return .handled
        }
        .onHover { isHovering = $0 }
        .help(metadataHelp)
        .contextMenu {
            Button("Copy event ID") {
                copyToPasteboard(entry.id)
            }
            Button(rawCopyLabel) {
                Task { await copyRawDetail() }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("p093-timeline-entry")
        .overlay(alignment: .topLeading) {
            VStack {
                P031AccessibilityMarker(identifier: "p093-timeline-entry-id-\(entry.id)")
                P031AccessibilityMarker(identifier: "p093-timeline-order-\(displayOrder)-\(entry.id)")
                if !isExpanded && entry.isTerminal && isResponseEntry {
                    P031AccessibilityMarker(identifier: "p093-timeline-collapsed-terminal-summary-\(entry.id)")
                }
                if isExpanded {
                    P031AccessibilityMarker(identifier: "p093-timeline-expanded-\(entry.id)")
                }
                if isExpanded && !expandedDetail.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    P031AccessibilityMarker(identifier: "p093-timeline-detail-non-empty")
                }
            }
        }
        .task(id: isExpanded) {
            if isExpanded {
                await resolveRawDetailIfNeeded()
            }
        }
        .id(entry.id)
        .transition(reduceMotion ? .opacity : .asymmetric(
            insertion: .push(from: .bottom).combined(with: .opacity),
            removal: .opacity
        ))
    }

    private var expandedControls: some View {
        HStack(spacing: 8) {
            Button {
                copyToPasteboard(entry.id)
            } label: {
                Label("Copy ID", systemImage: "doc.on.doc")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .accessibilityLabel("Copy Timeline event ID")
            .accessibilityIdentifier("p093-timeline-copy-id")

            Button {
                Task { await copyRawDetail() }
            } label: {
                Label(rawCopyLabel, systemImage: "doc.text")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .accessibilityLabel(rawCopyLabel)
            .disabled(isResolvingRawDetail)

            if let rawDetailStatusLabel {
                Text(rawDetailStatusLabel)
                    .font(.caption2)
                    .foregroundStyle(ForgeColor.Text.tertiary)
            }

            Spacer(minLength: 0)
        }
        .accessibilityIdentifier("p093-timeline-expanded-controls")
    }

    private var iconName: String {
        switch entry.kind {
        case .text: return "text.alignleft"
        case .mergedTool: return "hammer"
        case .sessionEvent: return "person.2.fill"
        case .agentSummary: return "doc.text.magnifyingglass"
        case .policyWarning: return "exclamationmark.shield"
        case .implementationCompletion: return "flag.checkered"
        case .persisted: return "tray.full"
        }
    }

    private var tint: Color {
        switch entry.kind {
        case .implementationCompletion: return ForgeStatusColor.success
        case .policyWarning: return ForgeStatusColor.approval
        case .sessionEvent: return ForgeStatusColor.running
        case .agentSummary: return ForgeStatusColor.neutral
        case .persisted: return ForgeColor.Brand.accent
        case .mergedTool: return ForgeStatusColor.warning
        case .text: return ForgeStatusColor.neutral
        }
    }

    private var previewDetail: String {
        guard isResponseEntry else { return entry.detail }
        let limit = 16_000
        guard entry.detail.count > limit else { return entry.detail }
        return "...\n" + String(entry.detail.suffix(limit))
    }

    private var collapsedDetail: String {
        guard isResponseEntry else { return previewDetail }
        if entry.isTerminal {
            return entry.detail
        }
        if entry.isStreaming {
            return streamingResponseSummary
        }
        return previewDetail
    }

    private var streamingResponseSummary: String {
        let chunks = entry.chunkCount ?? 1
        let chunkText = "\(chunks) chunk\(chunks == 1 ? "" : "s")"
        guard let bytes = entry.rawDetailBytes else {
            return "Response streaming · \(chunkText)"
        }
        return "Response streaming · \(chunkText) · \(bytes) byte\(bytes == 1 ? "" : "s") retained"
    }

    private var expandedDetail: String {
        resolvedFullRawDetail ?? retainedRawDetail
    }

    private var retainedRawDetail: String {
        entry.rawDetail?.isEmpty == false ? entry.rawDetail! : entry.detail
    }

    private var rawCopyLabel: String {
        resolvedFullRawDetail != nil || (entry.fullRawAvailable && !entry.rawDetailTruncated)
            ? "Copy full raw content"
            : "Copy retained raw content"
    }

    private var rawDetailStatusLabel: String? {
        if isResolvingRawDetail {
            return "Resolving full raw content"
        }
        if let rawDetailResolutionStatus, rawDetailResolutionStatus != .available {
            let reason = rawDetailResolutionErrorReason.map(rawDetailErrorLabel(for:))
                ?? rawDetailResolutionStatus.rawValue
            return "Full raw content unavailable: \(reason)"
        }
        if entry.rawDetailTruncated && resolvedFullRawDetail == nil {
            return "Full raw content unavailable"
        }
        if let rawDetailResolutionStatus {
            return "Raw detail: \(rawDetailResolutionStatus.rawValue)"
        }
        return nil
    }

    private func rawDetailErrorLabel(for reason: P031TimelineRawDetailErrorReason) -> String {
        switch reason {
        case .handleNotFound:
            return "handle_not_found"
        case .handleExpired:
            return "handle_expired"
        case .runNotAuthorized:
            return "run_not_authorized"
        case .eventNotAuthorized:
            return "event_not_authorized"
        case .storageUnavailable:
            return "storage_unavailable"
        case .digestValidationFailed:
            return "digest_mismatch"
        }
    }

    private var metadataText: some View {
        HStack(spacing: 8) {
            if let stageID = entry.stageID, !stageID.isEmpty {
                Text(stageID)
            }
            if let stateLabel = entry.stateLabel, !stateLabel.isEmpty {
                Text(stateLabel)
            }
            if let displayTime = entry.displayTime {
                Text(displayTime).monospacedDigit()
            }
            if let rawDetailBytes = entry.rawDetailBytes {
                Text("\(rawDetailBytes) bytes")
            }
            if entry.rawDetailTruncated {
                Text("truncated")
            }
            if let rawDetailResolutionStatus {
                Text(rawDetailResolutionStatus.rawValue)
            }
        }
        .accessibilityLabel(metadataHelp)
        .accessibilityIdentifier("p093-timeline-metadata")
    }

    private var metadataHelp: String {
        [
            entry.stageID.map { "State: \($0)" },
            entry.agentID.map { "Agent: \($0)" },
            entry.providerID.map { "Provider: \($0)" },
            entry.displayTime.map { "Time: \($0)" },
        ]
            .compactMap { $0 }
            .joined(separator: "\n")
    }

    private func shortID(_ id: String) -> String {
        guard id.count > 12 else { return id }
        return String(id.prefix(8)) + "..."
    }

    private func copyToPasteboard(_ value: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
    }

    @MainActor
    private func copyRawDetail() async {
        await resolveRawDetailIfNeeded()
        copyToPasteboard(expandedDetail)
    }

    @MainActor
    private func resolveRawDetailIfNeeded() async {
        guard resolvedFullRawDetail == nil,
              rawDetailResolutionStatus == nil,
              let handle = entry.rawDetailHandle?.trimmingCharacters(in: .whitespacesAndNewlines),
              !handle.isEmpty,
              entry.fullRawAvailable
        else {
            return
        }
        isResolvingRawDetail = true
        let result = await resolveTimelineRawDetail(handle)
        isResolvingRawDetail = false
        rawDetailResolutionStatus = result.status
        rawDetailResolutionErrorReason = result.errorReason
        guard result.status == .available,
              let rawDetail = result.rawDetail,
              result.rawDetailBytes == rawDetail.utf8.count,
              resolverDigestMatches(result.rawDetailDigest)
        else {
            if result.status == .available {
                rawDetailResolutionStatus = .digestMismatch
                rawDetailResolutionErrorReason = .digestValidationFailed
            }
            return
        }
        resolvedFullRawDetail = rawDetail
    }

    private func resolverDigestMatches(_ resolverDigest: String?) -> Bool {
        guard let expectedDigest = entry.rawDetailDigest?.trimmingCharacters(in: .whitespacesAndNewlines),
              !expectedDigest.isEmpty
        else {
            return true
        }
        return resolverDigest == expectedDigest
    }

    private var isResponseEntry: Bool {
        entry.surfaceLabel == "text_chunk"
            || entry.surfaceLabel == "agent_message_chunk"
            || entry.surfaceLabel == "agent_summary"
    }
}

@MainActor
private final class P093TimelineFormatterCache {
    private let maxCacheEntries = 32
    private var entries: [String: P093TimelineFormattedResult] = [:]
    private var accessOrder: [String] = []

    func render(
        event: RunsWorkbenchPresentationModel.TimelineEntry,
        detail: String,
        detailDigest: String?,
        detailCharCount: Int?,
        chunkCount: Int?
    ) -> P093TimelineFormattedResult {
        let key = cacheKey(
            event: event,
            detailDigest: detailDigest,
            detailCharCount: detailCharCount,
            chunkCount: chunkCount
        )
        if let cached = entries[key] {
            markRecentlyUsed(key)
            return cached
        }
        let rendered = P093TimelineFormattedResult.render(detail: detail, now: Date.init)
        entries[key] = rendered
        markRecentlyUsed(key)
        evictLeastRecentlyUsedEntry()
        return rendered
    }

    private func cacheKey(
        event: RunsWorkbenchPresentationModel.TimelineEntry,
        detailDigest: String?,
        detailCharCount: Int?,
        chunkCount: Int?
    ) -> String {
        let formatterVersion = P093TimelineFormattedResult.formatterVersion
        if let detailDigest, !detailDigest.isEmpty {
            return "\(event.id):\(detailDigest):\(formatterVersion)"
        }
        return "\(event.id):\(detailCharCount ?? 0):\(chunkCount ?? 0):\(formatterVersion)"
    }

    private func markRecentlyUsed(_ key: String) {
        accessOrder.removeAll { $0 == key }
        accessOrder.append(key)
    }

    private func evictLeastRecentlyUsedEntry() {
        while accessOrder.count > maxCacheEntries, let evicted = accessOrder.first {
            accessOrder.removeFirst()
            entries.removeValue(forKey: evicted)
        }
    }
}

struct P093TimelineFormattedResult: Equatable {
    static let formatterVersion = "p093-markdown-document-v1"
    static let formattedPreviewInputLimit = 96 * 1024
    static let jsonPrettyPrintInputLimit = 64 * 1024
    static let codeBlockPreviewLimit = 32 * 1024
    static let parseTimeFallbackLimitSeconds = 0.050

    let content: String
    let blocks: [Block]
    let previewTruncated: Bool
    let fallbackReason: FallbackReason?

    enum Block: Equatable {
        case text(String)
        case code(String)
    }

    enum FallbackReason: Equatable {
        case parseBudgetExceeded
    }

    static func expandedMinimumHeight(for detail: String) -> CGFloat {
        let trimmed = detail.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return 0 }
        let lineCount = max(1, min(trimmed.split(separator: "\n", omittingEmptySubsequences: false).count, 8))
        return min(max(CGFloat(lineCount * 22 + 36), 72), 220)
    }

    static func render(detail: String) -> P093TimelineFormattedResult {
        render(detail: detail, now: Date.init)
    }

    static func render(detail: String, now: () -> Date) -> P093TimelineFormattedResult {
        let startedAt = now()
        let budgeted = capped(detail, utf8Limit: formattedPreviewInputLimit)
        let normalizedContent = P093FormattedTimelineDetail.normalizedMarkdownContent(from: budgeted.text)
        let blocks = P093FormattedTimelineDetail.blocks(from: normalizedContent)
        if now().timeIntervalSince(startedAt) > parseTimeFallbackLimitSeconds {
            return P093TimelineFormattedResult(
                content: normalizedContent,
                blocks: [.text(normalizedContent)],
                previewTruncated: true,
                fallbackReason: .parseBudgetExceeded
            )
        }
        return P093TimelineFormattedResult(
            content: normalizedContent,
            blocks: blocks.isEmpty ? [.text(normalizedContent)] : blocks,
            previewTruncated: budgeted.truncated,
            fallbackReason: nil
        )
    }

    static func capped(_ text: String, utf8Limit: Int) -> (text: String, truncated: Bool) {
        guard text.utf8.count > utf8Limit else {
            return (text, false)
        }
        var capped = ""
        capped.reserveCapacity(utf8Limit)
        for character in text {
            if (capped.utf8.count + String(character).utf8.count) > utf8Limit {
                break
            }
            capped.append(character)
        }
        return (capped, true)
    }
}

private struct P093FormattedTimelineDetail: View {
    fileprivate let result: P093TimelineFormattedResult

    fileprivate init(result: P093TimelineFormattedResult) {
        self.result = result
    }

    var body: some View {
        stableScrollContainer
        .background(ForgeColor.Surface.muted.opacity(0.6), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(alignment: .topLeading) {
            if hasAccessibleContent {
                P031AccessibilityMarker(identifier: "p093-timeline-detail-non-empty")
                    .accessibilityLabel(accessibilitySummary)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilitySummary)
    }

    @ViewBuilder
    private var stableScrollContainer: some View {
        ScrollView {
            formattedContent
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var formattedContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            if result.previewTruncated {
                Label("Preview truncated", systemImage: "scissors")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(ForgeColor.Text.tertiary)
            }
            if result.fallbackReason == .parseBudgetExceeded {
                Label("Formatter budget fallback", systemImage: "timer")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(ForgeColor.Text.tertiary)
            }

            ForEach(Array(result.blocks.enumerated()), id: \.offset) { _, block in
                switch block {
                case .text(let text):
                    P093TimelineMarkdownTextBlock(text: text)
                case .code(let code):
                    P093TimelineCodeBlock(code: code)
                }
            }
        }
        .padding(8)
    }

    private var accessibilitySummary: String {
        let text = result.blocks
            .map { block -> String in
                switch block {
                case .text(let text), .code(let text):
                    return text
                }
            }
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return "Timeline detail preview is empty" }
        return String(text.prefix(1_000))
    }

    private var hasAccessibleContent: Bool {
        accessibilitySummary != "Timeline detail preview is empty"
    }

    fileprivate static func normalizedMarkdownContent(from detail: String) -> String {
        let normalizedNewlines = detail.replacingOccurrences(of: "\r\n", with: "\n")
        let lines = normalizedNewlines.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var output: [String] = []
        var inCodeFence = false

        func appendLine(_ line: String) {
            output.append(line)
        }

        func processTextLine(_ line: String) {
            guard let markerRange = line.range(of: "```") else {
                appendLine(line)
                return
            }

            let prefix = String(line[..<markerRange.lowerBound])
            if !prefix.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                appendLine(prefix)
            }

            let afterMarker = String(line[markerRange.upperBound...])
            let parsed = parseOpeningFenceRemainder(afterMarker, forceSplitPayload: !prefix.isEmpty)
            appendLine("```" + parsed.info)
            inCodeFence = true
            if let payload = parsed.payload, !payload.isEmpty {
                appendLine(payload)
            }
        }

        func processCodeLine(_ line: String) {
            guard let markerRange = line.range(of: "```") else {
                appendLine(line)
                return
            }

            let codePrefix = String(line[..<markerRange.lowerBound])
            if !codePrefix.isEmpty {
                appendLine(codePrefix)
            }
            appendLine("```")
            inCodeFence = false

            let trailing = String(line[markerRange.upperBound...])
                .trimmingCharacters(in: .whitespacesAndNewlines)
            if !trailing.isEmpty {
                processTextLine(trailing)
            }
        }

        for line in lines {
            if inCodeFence {
                processCodeLine(line)
            } else {
                processTextLine(line)
            }
        }

        return output.joined(separator: "\n")
    }

    private static func parseOpeningFenceRemainder(
        _ remainder: String,
        forceSplitPayload: Bool
    ) -> (info: String, payload: String?) {
        let trimmedLeading = remainder.trimmingCharacters(in: .whitespaces)
        guard !trimmedLeading.isEmpty else {
            return ("", nil)
        }

        guard forceSplitPayload else {
            return (trimmedLeading, nil)
        }

        let pieces = trimmedLeading.split(maxSplits: 1, whereSeparator: { $0 == " " || $0 == "\t" })
        guard let first = pieces.first else {
            return ("", nil)
        }

        let language = String(first)
        guard isLikelyFenceLanguage(language) else {
            return ("", trimmedLeading)
        }

        let payload = pieces.count > 1
            ? String(pieces[1]).trimmingCharacters(in: .whitespaces)
            : nil
        return (language, payload?.isEmpty == true ? nil : payload)
    }

    private static func isLikelyFenceLanguage(_ value: String) -> Bool {
        guard !value.isEmpty, value.count <= 32 else { return false }
        return value.allSatisfy { character in
            character.isLetter
                || character.isNumber
                || character == "_"
                || character == "-"
                || character == "+"
                || character == "#"
                || character == "."
        }
    }

    fileprivate static func blocks(from detail: String) -> [P093TimelineFormattedResult.Block] {
        let lines = detail.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var blocks: [P093TimelineFormattedResult.Block] = []
        var current: [String] = []
        var code: [String] = []
        var inCode = false

        func flushText() {
            let text = current.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty {
                blocks.append(contentsOf: plainTextBlocks(from: text))
            }
            current.removeAll()
        }

        func flushCode() {
            let text = code.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty {
                let capped = P093TimelineFormattedResult.capped(
                    text,
                    utf8Limit: P093TimelineFormattedResult.codeBlockPreviewLimit
                )
                blocks.append(.code(capped.truncated ? capped.text + "\nPreview truncated" : capped.text))
            }
            code.removeAll()
        }

        for line in lines {
            if line.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                if inCode {
                    flushCode()
                } else {
                    flushText()
                }
                inCode.toggle()
                continue
            }
            if inCode {
                code.append(line)
            } else {
                current.append(line)
            }
        }

        if inCode {
            flushCode()
        } else {
            flushText()
        }
        return blocks.isEmpty ? [.text(detail)] : blocks
    }

    private static func plainTextBlocks(from text: String) -> [P093TimelineFormattedResult.Block] {
        if let prettyJSON = prettyPrintedJSON(text) {
            return [.code(prettyJSON)]
        }

        if looksLikeChainworksOutputJSON(text) {
            let capped = P093TimelineFormattedResult.capped(
                text.trimmingCharacters(in: .whitespacesAndNewlines),
                utf8Limit: P093TimelineFormattedResult.codeBlockPreviewLimit
            )
            return [.code(capped.truncated ? capped.text + "\nPreview truncated" : capped.text)]
        }

        if let markerBlocks = chainworksOutputBlocks(from: text) {
            return markerBlocks
        }

        return [.text(text)]
    }

    private static func chainworksOutputBlocks(from text: String) -> [P093TimelineFormattedResult.Block]? {
        guard let markerRange = chainworksOutputMarkerRange(in: text) else {
            return nil
        }

        var blocks: [P093TimelineFormattedResult.Block] = []
        let preface = String(text[..<markerRange.lowerBound])
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if !preface.isEmpty {
            blocks.append(.text(preface))
        }
        blocks.append(.text("CHAINWORKS_OUTPUT"))

        let payload = String(text[markerRange.upperBound...])
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !payload.isEmpty else {
            return blocks
        }

        if let prettyJSON = prettyPrintedJSON(payload) {
            blocks.append(.code(prettyJSON))
        } else if let extracted = firstPrettyPrintedJSONFragment(in: payload) {
            let prefix = String(payload[..<extracted.range.lowerBound])
                .trimmingCharacters(in: .whitespacesAndNewlines)
            if !prefix.isEmpty {
                blocks.append(.text(prefix))
            }
            blocks.append(.code(extracted.prettyJSON))
            let suffix = String(payload[extracted.range.upperBound...])
                .trimmingCharacters(in: .whitespacesAndNewlines)
            if !suffix.isEmpty {
                blocks.append(.text(suffix))
            }
        } else {
            let capped = P093TimelineFormattedResult.capped(
                payload,
                utf8Limit: P093TimelineFormattedResult.codeBlockPreviewLimit
            )
            blocks.append(.code(capped.truncated ? capped.text + "\nPreview truncated" : capped.text))
        }
        return blocks
    }

    private static func looksLikeChainworksOutputJSON(_ text: String) -> Bool {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.hasPrefix(#"{"CHAINWORKS_OUTPUT""#)
            || trimmed.hasPrefix(#"{ "CHAINWORKS_OUTPUT""#)
    }

    private static func chainworksOutputMarkerRange(in text: String) -> Range<String.Index>? {
        var cursor = text.startIndex
        while cursor < text.endIndex {
            let lineEnd = text[cursor...].firstIndex(of: "\n") ?? text.endIndex
            let lineRange = cursor..<lineEnd
            let trimmed = text[lineRange].trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed == "CHAINWORKS_OUTPUT" || trimmed == "CHAINWORKS_OUTPUT:" {
                return lineRange
            }
            cursor = lineEnd == text.endIndex ? text.endIndex : text.index(after: lineEnd)
        }
        return nil
    }

    private static func firstPrettyPrintedJSONFragment(
        in text: String
    ) -> (range: Range<String.Index>, prettyJSON: String)? {
        for start in text.indices where text[start] == "{" || text[start] == "[" {
            guard let end = matchingJSONEnd(in: text, startingAt: start) else {
                continue
            }
            let candidateRange = start..<text.index(after: end)
            let candidate = String(text[candidateRange])
            if let prettyJSON = prettyPrintedJSON(candidate) {
                return (candidateRange, prettyJSON)
            }
        }
        return nil
    }

    private static func matchingJSONEnd(in text: String, startingAt start: String.Index) -> String.Index? {
        var stack: [Character] = []
        var isInString = false
        var isEscaped = false
        var index = start
        while index < text.endIndex {
            let character = text[index]
            if isInString {
                if isEscaped {
                    isEscaped = false
                } else if character == "\\" {
                    isEscaped = true
                } else if character == "\"" {
                    isInString = false
                }
            } else {
                switch character {
                case "\"":
                    isInString = true
                case "{":
                    stack.append("}")
                case "[":
                    stack.append("]")
                case "}", "]":
                    guard stack.last == character else {
                        return nil
                    }
                    stack.removeLast()
                    if stack.isEmpty {
                        return index
                    }
                default:
                    break
                }
            }
            index = text.index(after: index)
        }
        return nil
    }

    private static func prettyPrintedJSON(_ text: String) -> String? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("{") || trimmed.hasPrefix("["),
              trimmed.utf8.count <= P093TimelineFormattedResult.jsonPrettyPrintInputLimit,
              let data = trimmed.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data),
              JSONSerialization.isValidJSONObject(object),
              let pretty = try? JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys]),
              let rendered = String(data: pretty, encoding: .utf8)
        else {
            return nil
        }
        return rendered
    }
}

private struct P093TimelineMarkdownTextBlock: View {
    let text: String

    var body: some View {
        Text(renderedText)
            .font(.system(size: 13))
            .foregroundStyle(ForgeColor.Text.secondary)
            .lineSpacing(3)
            .textSelection(.enabled)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityLabel(text)
    }

    private var renderedText: AttributedString {
        if let attributed = try? AttributedString(
            markdown: text,
            options: .init(interpretedSyntax: .full)
        ) {
            return attributed
        }
        return AttributedString(text)
    }
}

private struct P093TimelineCodeBlock: View {
    let code: String

    var body: some View {
        Text(code)
            .font(.system(size: 12, design: .monospaced))
            .foregroundStyle(ForgeColor.Text.secondary)
            .textSelection(.enabled)
            .lineSpacing(2)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
            .background(Color(nsColor: .textBackgroundColor).opacity(0.72), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

private struct P031RunDetailSummaryCard: View {
    let header: RunsWorkbenchPresentationModel.SummaryHeader
    let onCompactCloseoutActivated: () -> Void
    let onCheckSystemReadiness: () -> Void

    var body: some View {
        P031CalloutCard(
            title: header.title,
            bodyText: detailBody,
            accentColor: .accentColor
        ) {
            HStack(spacing: 10) {
                P031RunsHomeAccessibilityMarker(
                    identifier: "p031-run-detail-summary-\(header.freshness)",
                    label: header.title
                )
                if let errorDescription = header.errorDescription {
                    ForgeWarningBanner.error(errorDescription)
                }

                if header.status == "blocked" || header.status == "failed" {
                    Button {
                        onCheckSystemReadiness()
                    } label: {
                        Label("Check system readiness", systemImage: "stethoscope")
                            .font(.subheadline.weight(.semibold))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
        }
    }

    private var detailBody: String {
        [
            header.workflowLabel,
            header.status,
            header.rolloutDecisionSummary,
            header.progressLabel,
            header.pendingApprovalsLabel,
            header.refreshFeedbackText,
        ]
        .compactMap { $0 }
        .joined(separator: " • ")
    }
}

private struct P086ContinuationReadbackCard: View {
    let presentation: P086ContinuationReadbackPresentation

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForgeSectionHeader(
                title: presentation.title,
                subtitle: presentation.summary,
                symbol: "arrow.triangle.2.circlepath"
            )

            HStack(alignment: .top, spacing: 12) {
                Label(presentation.latestStatus, systemImage: statusSymbolName)
                    .font(ForgeTypography.supporting.weight(.semibold))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(statusColor.opacity(0.15), in: Capsule())
                    .foregroundStyle(statusColor)

                VStack(alignment: .leading, spacing: 6) {
                    Text(presentation.latestMode)
                        .font(ForgeTypography.body.weight(.semibold))
                        .foregroundStyle(ForgeColor.Text.primary)
                    Text([
                        presentation.latestTrigger,
                        presentation.artifactSummary,
                        presentation.metricSummary,
                    ].joined(separator: " · "))
                    .font(ForgeTypography.supporting)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
            }

            LazyVGrid(columns: [GridItem(.adaptive(minimum: 220), spacing: 10)], spacing: 8) {
                if let id = presentation.latestContinuationID {
                    P086ContinuationMetadataPill(label: "Continuation", value: id)
                }
                if let id = presentation.latestAgentExecutionID {
                    P086ContinuationMetadataPill(label: "Agent execution", value: id)
                }
                if let id = presentation.latestStageExecutionID {
                    P086ContinuationMetadataPill(label: "Stage execution", value: id)
                }
            }
        }
        .forgePanel()
        .accessibilityIdentifier("p086-continuation-readback-card")
        .accessibilityLabel(presentation.accessibilityLabel)
    }

    private var statusColor: Color {
        let status = presentation.latestStatus.lowercased()
        if status.contains("succeeded") { return ForgeColor.Status.success }
        if status.contains("failed") || status.contains("no progress") { return ForgeColor.Status.error }
        if status.contains("cancel") { return ForgeColor.Status.warning }
        return ForgeColor.Status.running
    }

    private var statusSymbolName: String {
        let status = presentation.latestStatus.lowercased()
        if status.contains("succeeded") { return "checkmark.circle.fill" }
        if status.contains("failed") || status.contains("no progress") { return "exclamationmark.triangle.fill" }
        if status.contains("cancel") { return "xmark.circle.fill" }
        return "arrow.triangle.2.circlepath"
    }
}

private struct P086ContinuationMetadataPill: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(ForgeTypography.micro.weight(.semibold))
                .foregroundStyle(ForgeColor.Text.tertiary)
            Text(value)
                .font(ForgeTypography.micro.monospaced())
                .foregroundStyle(ForgeColor.Text.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(ForgeColor.Surface.muted, in: RoundedRectangle(cornerRadius: 8))
    }
}

private struct P088ImplementationCompletionCard: View {
    let presentation: P088ImplementationCompletionPresentation
    @State private var copyFeedback: String?

    var body: some View {
        P031CalloutCard(
            title: "Implementation Completion",
            bodyText: presentation.outputFreshnessLabel,
            accentColor: accentColor
        ) {
            VStack(alignment: .leading, spacing: 10) {
                Label(presentation.statusLabel, systemImage: statusSymbolName)
                    .font(.caption.weight(.semibold))
                    .accessibilityIdentifier("p088-implementation-completion-status")

                if let failureClassLabel = presentation.failureClassLabel {
                    Label(failureClassLabel, systemImage: "xmark.octagon")
                        .font(.caption)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("p088-implementation-completion-failure-class")
                }

                if let workChangeKindLabel = presentation.workChangeKindLabel {
                    Label(workChangeKindLabel, systemImage: "arrow.triangle.branch")
                        .font(.caption)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("p088-implementation-completion-work-change-kind")
                }

                Label(presentation.outputFreshnessLabel, systemImage: "checklist")
                    .font(.caption)
                    .accessibilityIdentifier("p088-implementation-completion-output-freshness")

                if let evidencePathLabel = presentation.evidencePathLabel {
                    Label(evidencePathLabel, systemImage: "doc.text.magnifyingglass")
                        .font(.caption)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("p088-implementation-completion-evidence-path")
                }

                Label(presentation.nextOperatorActionLabel, systemImage: "arrow.right.circle")
                    .font(.caption.weight(.medium))
                    .accessibilityIdentifier("p088-implementation-completion-next-action")

                if !presentation.diagnosticRows.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(presentation.diagnosticRows, id: \.self) { row in
                            Text(row)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                    .accessibilityIdentifier("p088-implementation-completion-diagnostics")
                }

                if !presentation.copyItems.isEmpty {
                    HStack(spacing: 8) {
                        ForEach(presentation.copyItems, id: \.label) { item in
                            Button {
                                copy(item)
                            } label: {
                                Label(item.label, systemImage: "doc.on.doc")
                            }
                            .controlSize(.small)
                        }
                    }
                }

                if let copyFeedback {
                    Text(copyFeedback)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("p088-implementation-completion-copy-feedback")
                }
            }
            .accessibilityLabel(presentation.accessibilityLabel)
            .accessibilityIdentifier("p088-implementation-completion-card")
        }
    }

    private var accentColor: Color {
        switch presentation.visualState {
        case .positive:
            return .green
        case .warning:
            return .orange
        case .blocking:
            return .red
        case .neutral:
            return .secondary
        }
    }

    private var statusSymbolName: String {
        switch presentation.visualState {
        case .positive:
            return "checkmark.seal"
        case .warning:
            return "exclamationmark.triangle"
        case .blocking:
            return "xmark.octagon"
        case .neutral:
            return "questionmark.circle"
        }
    }

    private func copy(_ item: P031DiagnosticCopyItem) {
#if os(macOS)
        NSPasteboard.general.clearContents()
        let didCopy = NSPasteboard.general.setString(item.value, forType: .string)
        copyFeedback = didCopy ? "Copied \(item.label.lowercased())" : "Copy failed"
#else
        copyFeedback = "Copied \(item.label.lowercased())"
#endif
    }
}

private struct P078SideEffectReadbackCard: View {
    let presentation: P078SideEffectReadbackPresentation
    @State private var copyFeedback: String?

    var body: some View {
        P031CalloutCard(
            title: "Release Side Effects",
            bodyText: presentation.statusLabel,
            accentColor: presentation.visualState == .blocking ? .red : .secondary
        ) {
            VStack(alignment: .leading, spacing: 10) {
                Label(presentation.nextOperatorActionLabel, systemImage: "arrow.right.circle")
                    .font(.caption.weight(.medium))
                    .accessibilityIdentifier("p078-side-effect-next-action")

                if !presentation.diagnosticRows.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(presentation.diagnosticRows, id: \.self) { row in
                            Text(row)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                    .accessibilityIdentifier("p078-side-effect-diagnostics")
                }

                if !presentation.copyItems.isEmpty {
                    HStack(spacing: 8) {
                        ForEach(presentation.copyItems, id: \.label) { item in
                            Button {
                                copy(item)
                            } label: {
                                Label(item.label, systemImage: "doc.on.doc")
                            }
                            .controlSize(.small)
                        }
                    }
                }

                if let copyFeedback {
                    Text(copyFeedback)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("p078-side-effect-copy-feedback")
                }
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(presentation.accessibilityLabel)
            .accessibilityIdentifier("p078-side-effect-readback-card")
        }
    }

    private func copy(_ item: P031DiagnosticCopyItem) {
#if os(macOS)
        NSPasteboard.general.clearContents()
        let didCopy = NSPasteboard.general.setString(item.value, forType: .string)
        copyFeedback = didCopy ? "Copied \(item.label.lowercased())" : "Copy failed"
#else
        copyFeedback = "Copied \(item.label.lowercased())"
#endif
    }
}

private struct P077CloseoutReadinessCard: View {
    let presentation: P077CloseoutReadinessPresentation
    let closeoutFocus: FocusState<P077CloseoutReadinessFocus?>.Binding
    let onReturnToCloseoutReadiness: () -> Void
    @State private var copyFeedback: String?
    @State private var isDiagnosticsPresented = false
    @State private var announcementState = P077CloseoutReadinessAnnouncementState()
    @State private var latestAnnouncement: P077CloseoutReadinessAnnouncement?

    var body: some View {
        P031CalloutCard(
            title: "Closeout Readiness",
            bodyText: presentation.detailText,
            accentColor: accentColor
        ) {
            VStack(alignment: .leading, spacing: 10) {
                P031RunsHomeAccessibilityMarker(
                    identifier: "p077-closeout-readiness-card",
                    label: presentation.cardAccessibilityLabel,
                    value: latestAnnouncement.map {
                        "\($0.priority.accessibilityPriorityLabel): \($0.text)"
                    }
                )
                P031RunsHomeAccessibilityMarker(
                    identifier: "p077-closeout-readiness-announcement-priority",
                    label: presentation.voiceOverAnnouncementPolicy,
                    value: latestAnnouncement?.priority.accessibilityPriorityLabel
                )

                HStack(spacing: 8) {
                    P077CompactSignalCapsule(
                        label: presentation.compactSignalLabel,
                        systemImage: statusSymbolName,
                        accentColor: accentColor,
                        accessibilityLabel: presentation.compactActivationAccessibilityLabel,
                        accessibilityIdentifier: "p077-closeout-readiness-compact-status"
                    )
                    .focused(closeoutFocus, equals: .compactSignal)
                    Spacer()
                }

                HStack(spacing: 8) {
                    Label(presentation.statusLabel, systemImage: statusSymbolName)
                        .font(.caption.weight(.semibold))
                    Text(presentation.modeLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button {
                        isDiagnosticsPresented = true
                    } label: {
                        Label("Diagnostics", systemImage: "stethoscope")
                    }
                    .controlSize(.small)
                    .accessibilityLabel(presentation.diagnosticsAccessibilityLabel)
                    .accessibilityIdentifier("p077-closeout-readiness-diagnostics")
                    .focused(closeoutFocus, equals: .diagnosticsTrigger)

                    Button {
                        copyGenerationID()
                    } label: {
                        Label("Copy generation id", systemImage: "doc.on.doc")
                    }
                    .controlSize(.small)
                    .disabled(presentation.generationCopyValue == nil)
                    .accessibilityLabel(presentation.generationCopyAccessibilityLabel)
                    .accessibilityIdentifier("p077-closeout-readiness-generation-copy")
                    .focused(closeoutFocus, equals: .copyGeneration)
                }

                Label(presentation.primaryUnblockText, systemImage: "exclamationmark.circle")
                    .font(.callout.weight(.medium))
                    .focusable(true)
                    .focused(closeoutFocus, equals: .primaryUnblock)
                    .accessibilityLabel("Primary unblock: \(presentation.primaryUnblockText)")
                    .accessibilityIdentifier("p077-closeout-readiness-primary-unblock")

                if !presentation.secondaryBlockerRows.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(presentation.secondaryBlockerRows, id: \.self) { row in
                            Label(row, systemImage: "smallcircle.filled.circle")
                                .font(.caption)
                                .focusable(true)
                                .focused(closeoutFocus, equals: .secondaryBlocker(row))
                                .accessibilityLabel("Queued behind \(presentation.primaryUnblockText): \(row)")
                        }
                    }
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("p077-closeout-readiness-secondary-blockers")
                }

                recoveryLifecycleSection

                Label(presentation.backlinkRouteLabel, systemImage: "arrowshape.turn.up.right")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .focusable(true)
                    .focused(closeoutFocus, equals: .backlinkRoute)
                    .accessibilityLabel(presentation.backlinkRouteAccessibilityLabel)
                    .accessibilityIdentifier("p077-closeout-readiness-backlink-route")

                Label(presentation.modeExplainerText, systemImage: "info.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .focusable(true)
                    .focused(closeoutFocus, equals: .modeExplainer)
                    .accessibilityLabel(presentation.modeExplainerAccessibilityLabel)
                    .accessibilityIdentifier("p077-closeout-readiness-mode-explainer")

                if let copyFeedback {
                    Text(copyFeedback)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .focusable(true)
                        .focused(closeoutFocus, equals: .copyFallback)
                        .focused(closeoutFocus, equals: .recoveryCopyFeedback)
                        .accessibilityIdentifier("p077-closeout-readiness-copy-fallback")
                }
            }
        }
        .onAppear {
            recordCloseoutAnnouncement(sheetOwnsFocus: isDiagnosticsPresented)
        }
        .onChange(of: presentation) {
            recordCloseoutAnnouncement(sheetOwnsFocus: isDiagnosticsPresented)
        }
        .onChange(of: isDiagnosticsPresented) {
            recordCloseoutAnnouncement(sheetOwnsFocus: isDiagnosticsPresented)
        }
        .sheet(
            isPresented: $isDiagnosticsPresented,
            onDismiss: { closeoutFocus.wrappedValue = .diagnosticsTrigger }
        ) {
            P077CloseoutReadinessDiagnosticsSheet(
                presentation: presentation,
                onReturnToCloseoutReadiness: {
                    isDiagnosticsPresented = false
                    onReturnToCloseoutReadiness()
                }
            )
        }
    }

    private var accentColor: Color {
        switch presentation.visualState {
        case .positive, .warning, .blocking, .neutral:
            return P077CloseoutReadinessChrome.accentColor(for: presentation.visualState)
        }
    }

    private var statusSymbolName: String {
        switch presentation.visualState {
        case .positive:
            return "checkmark.seal"
        case .warning:
            return "exclamationmark.triangle"
        case .blocking:
            return "xmark.octagon"
        case .neutral:
            return "minus.circle"
        }
    }

    private func copyGenerationID() {
        guard let value = presentation.generationCopyValue else {
            copyFeedback = "No generation id available"
            return
        }
#if os(macOS)
        NSPasteboard.general.clearContents()
        let didCopy = NSPasteboard.general.setString(value, forType: .string)
        copyFeedback = didCopy
            ? "Copied generation \(presentation.generationDisplayID)"
            : presentation.copyFailureFallbackText
        if !didCopy {
            closeoutFocus.wrappedValue = .copyFallback
        }
#else
        copyFeedback = "Copied generation \(presentation.generationDisplayID)"
#endif
    }

    private var recoveryLifecycleSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(presentation.recoveryLifecycleText, systemImage: "arrow.clockwise.circle")
                .font(.caption.weight(.semibold))
                .focusable(true)
                .focused(closeoutFocus, equals: .recoveryLifecycle)
                .accessibilityIdentifier("p077-closeout-readiness-recovery-non-dismissible")

            Text(presentation.recoveryLifecycleAcknowledgementText)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(presentation.recoveryLifecycleCorrelationText)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(presentation.recoveryLifecycleFreshnessBudgetText)
                .font(.caption)
                .foregroundStyle(.secondary)

            ForEach(presentation.recoveryLifecycleActionRows, id: \.self) { action in
                Label(action, systemImage: "arrow.right.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Button {
                copyRecoveryTemplate()
            } label: {
                Label("Copy recovery template", systemImage: "doc.on.clipboard")
            }
            .controlSize(.small)
            .accessibilityLabel("Copy P077 stalled recovery escalation template")
            .accessibilityIdentifier("p077-closeout-readiness-recovery-copy-template")
            .focused(closeoutFocus, equals: .copyRecoveryTemplate)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(presentation.recoveryLifecycleAccessibilityLabel)
        .accessibilityIdentifier("p077-closeout-readiness-recovery")
    }

    private func copyRecoveryTemplate() {
#if os(macOS)
        NSPasteboard.general.clearContents()
        let didCopy = NSPasteboard.general.setString(
            presentation.recoveryLifecycleCopyTemplate,
            forType: .string
        )
        copyFeedback = didCopy
            ? "Copied recovery template for generation \(presentation.generationDisplayID)"
            : "Copy failed. Recovery template remains visible in diagnostics."
        closeoutFocus.wrappedValue = didCopy ? .recoveryLifecycle : .recoveryCopyFeedback
#else
        copyFeedback = "Copied recovery template for generation \(presentation.generationDisplayID)"
        closeoutFocus.wrappedValue = .recoveryLifecycle
#endif
    }

    private func recordCloseoutAnnouncement(sheetOwnsFocus: Bool) {
        var state = announcementState
        let announcement = P077CloseoutReadinessAnnouncementPolicy.announcement(
            for: presentation,
            previous: &state,
            now: Date(),
            sheetOwnsFocus: sheetOwnsFocus
        )
        latestAnnouncement = announcement
        announcementState = state
        if let announcement {
            postCloseoutAccessibilityAnnouncement(announcement)
        }
    }

    private func postCloseoutAccessibilityAnnouncement(
        _ announcement: P077CloseoutReadinessAnnouncement
    ) {
#if os(macOS)
        let priority: NSAccessibilityPriorityLevel =
            announcement.priority == .assertive ? .high : .medium
        let element: Any = NSApp.keyWindow ?? NSApp as Any
        NSAccessibility.post(
            element: element,
            notification: .announcementRequested,
            userInfo: [
                .announcement: announcement.text,
                .priority: priority.rawValue,
            ]
        )
#endif
    }
}

private struct P031RunsHomeAccessibilityMarker: View {
    let identifier: String
    let label: String
    var value: String? = nil

    var body: some View {
        Text(" ")
            .font(.system(size: 1))
            .frame(width: 1, height: 1)
            .foregroundStyle(.clear)
            .accessibilityLabel(label)
            .accessibilityValue(value ?? "")
            .accessibilityIdentifier(identifier)
    }
}

private enum P077CloseoutReadinessChrome {
    static func accentColor(for visualState: P077CloseoutReadinessVisualState) -> Color {
        switch visualState {
        case .positive:
            return .green
        case .warning:
            return .orange
        case .blocking:
            return .red
        case .neutral:
            return .secondary
        }
    }
}

private struct P077CompactSignalCapsule: View {
    let label: String
    let systemImage: String
    let accentColor: Color
    let accessibilityLabel: String
    let accessibilityIdentifier: String
    var action: (() -> Void)? = nil

    var body: some View {
        if let action {
            Button(action: action) {
                capsuleContent
            }
            .buttonStyle(.plain)
            .accessibilityLabel(accessibilityLabel)
            .accessibilityIdentifier(accessibilityIdentifier)
        } else {
            capsuleContent
                .accessibilityLabel(accessibilityLabel)
                .accessibilityIdentifier(accessibilityIdentifier)
        }
    }

    private var capsuleContent: some View {
        Label(label, systemImage: systemImage)
            .font(.caption.weight(.semibold))
            .foregroundStyle(.primary)
            .lineLimit(1)
            .minimumScaleFactor(0.85)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(accentColor.opacity(0.16), in: Capsule())
            .overlay(
                Capsule()
                    .stroke(accentColor.opacity(0.35), lineWidth: 1)
            )
    }
}

private struct P077CloseoutReadinessDiagnosticsSheet: View {
    let presentation: P077CloseoutReadinessPresentation
    let onReturnToCloseoutReadiness: () -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Button {
                    dismiss()
                    DispatchQueue.main.async {
                        onReturnToCloseoutReadiness()
                    }
                } label: {
                    Label("Closeout Readiness", systemImage: "chevron.left")
                }
                .keyboardShortcut(.cancelAction)
                .accessibilityIdentifier("p077-closeout-readiness-return")

                Text("Closeout Diagnostics")
                    .font(.headline)
                Spacer()
                Button("Done") {
                    dismiss()
                }
            }

            ForEach(presentation.diagnosticRows, id: \.self) { row in
                Label(row, systemImage: "checklist")
                    .font(.callout)
            }

            Text(presentation.focusReturnLabel)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(20)
        .frame(minWidth: 420, idealWidth: 480, maxWidth: 560, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(presentation.diagnosticsAccessibilityLabel)
        .accessibilityIdentifier("p077-closeout-readiness-diagnostics-sheet")
    }
}

private struct P031IdeaContextCard: View {
    let presentation: P031IdeaContextPresentation?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Run context")
                .font(.headline)
            if let presentation {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text(presentation.title)
                            .font(.title3.weight(.semibold))
                            .lineLimit(2)
                        Spacer()
                        if let statusLabel = presentation.statusLabel {
                            Text(statusLabel)
                                .font(.caption.weight(.semibold))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(Color.green.opacity(0.12), in: Capsule())
                        }
                    }
                    if let body = presentation.body, !body.isEmpty {
                        Text(body)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .lineLimit(4)
                    }
                    P031BadgeRow(
                        labels: [
                            presentation.projectKey.map { "Project: \($0)" },
                            presentation.createdAt.map { "Created: \($0)" },
                            presentation.archivedAt.map { "Archived: \($0)" },
                        ].compactMap { $0 }
                    )
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                .accessibilityLabel(presentation.accessibilityLabel)
            } else {
                ForgeEmptyState(
                    title: "Run context unavailable",
                    systemImage: "lightbulb",
                    description: "The selected run did not include a GraphQL-readable run reference."
                )
            }
        }
    }
}

private struct P031StageTransitionMapCard: View {
    let stages: [RunsWorkbenchPresentationModel.StageCard]
    let onArtifactsSelected: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Stage transitions")
                .font(.headline)
            if stages.isEmpty {
                ForgeEmptyState(
                    title: "No transitions",
                    systemImage: "list.bullet",
                    description: "No stage projections returned."
                )
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(stages.enumerated()), id: \.element.id) { index, stage in
                        HStack(alignment: .top, spacing: 12) {
                            VStack(spacing: 0) {
                                Circle()
                                    .fill(color(for: stage.status))
                                    .frame(width: 12, height: 12)
                                    .overlay(Circle().stroke(.white.opacity(0.85), lineWidth: 1))
                                if index < stages.count - 1 {
                                    Rectangle()
                                        .fill(color(for: stage.status).opacity(0.45))
                                        .frame(width: 2, height: 42)
                                }
                            }
                            VStack(alignment: .leading, spacing: 6) {
                                HStack(alignment: .firstTextBaseline) {
                                    Text(stage.title)
                                        .font(.subheadline.weight(.semibold))
                                    Spacer()
                                    Text(statusLabel(for: stage.status))
                                        .font(.caption.weight(.medium))
                                        .foregroundStyle(color(for: stage.status))
                                }
                                if let attemptText = stage.attemptText {
                                    Text(attemptText)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let startedLabel = stage.startedLabel {
                                    Text(startedLabel)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let completedLabel = stage.completedLabel {
                                    Text(completedLabel)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let durationLabel = stage.durationLabel {
                                    Text(durationLabel)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }

                                if !stage.evidenceLabels.isEmpty {
                                    P031BadgeRow(labels: stage.evidenceLabels)
                                }
                                if stage.artifactCount > 0 {
                                    Button {
                                        onArtifactsSelected(stage.id)
                                    } label: {
                                        HStack(spacing: 5) {
                                            Image(systemName: "doc.text.magnifyingglass")
                                            Text("\(stage.artifactCount) artifact\(stage.artifactCount == 1 ? "" : "s")")
                                        }
                                        .font(.caption.weight(.medium))
                                        .foregroundStyle(Color.accentColor)
                                        .padding(.horizontal, 10)
                                        .padding(.vertical, 5)
                                        .background(Color.accentColor.opacity(0.14), in: Capsule())
                                    }
                                    .buttonStyle(.borderless)
                                    .controlSize(.small)
                                }
                            }
                            .padding(.bottom, index < stages.count - 1 ? 16 : 0)
                        }
                    }
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
            }
        }
    }

    private func color(for status: String) -> Color {
        switch status {
        case "terminal": return .green
        case "active": return .blue
        case "blocked": return .red
        case "pending": return .orange
        case "unavailable": return .secondary
        default: return .secondary
        }
    }

    private func statusLabel(for status: String) -> String {
        switch status {
        case "terminal": return "Completed"
        case "active": return "Running"
        case "blocked": return "Blocked"
        case "pending": return "Pending"
        case "unavailable": return "Unavailable"
        default: return status.capitalized
        }
    }
}

private struct P031ApprovalInboxCard: View {
    let presentation: P031ApprovalInboxPresentation?
    let actionError: String?
    let isResolving: (String) -> Bool
    let onApprove: (String) -> Void
    let onReject: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Approvals")
                    .font(.headline)
                Spacer()
                if let presentation {
                    P031FreshnessBadge(snapshot: presentation.freshness)
                }
            }
            if let actionError {
                ForgeWarningBanner.error(actionError)
            }
            if let presentation {
                if presentation.rows.isEmpty {
                    ForgeEmptyState(
                        title: presentation.emptyStateTitle ?? "No pending approvals",
                        systemImage: "checkmark.seal",
                        description: presentation.errorDescription ?? presentation.refreshFeedbackText
                    )
                } else {
                    ForEach(presentation.rows, id: \.approvalID) { row in
                        P031CalloutCard(
                            title: row.title,
                            bodyText: row.body,
                            accentColor: .orange
                        ) {
                            VStack(alignment: .leading, spacing: 10) {
                                if row.canApprove || row.canReject {
                                    HStack(spacing: 8) {
                                        if row.canApprove {
                                            Button {
                                                onApprove(row.approvalID)
                                            } label: {
                                                Label("Approve", systemImage: "checkmark.circle")
                                            }
                                            .disabled(isResolving(row.approvalID))
                                            .controlSize(.small)
                                        }
                                        if row.canReject {
                                            Button(role: .destructive) {
                                                onReject(row.approvalID)
                                            } label: {
                                                Label("Reject", systemImage: "xmark.circle")
                                            }
                                            .disabled(isResolving(row.approvalID))
                                            .controlSize(.small)
                                        }
                                        if isResolving(row.approvalID) {
                                            ProgressView()
                                                .controlSize(.small)
                                                .accessibilityLabel("Resolving approval")
                                        }
                                    }
                                } else if let actionLabel = row.actionLabel {
                                    Label(actionLabel, systemImage: "terminal")
                                        .font(.caption)
                                }

                                if let state = row.deferredState {
                                    ForgeWarningBanner(state.displayLabel, tint: state.tint)
                                }
                                if let followUpID = row.followUpID {
                                    Text(followUpID)
                                        .font(.caption.monospaced())
                                        .foregroundStyle(.secondary)
                                }
                                P031CopyItemsView(items: row.copyItems)
                            }
                        }
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel(row.accessibilityLabel)
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    ForgeSkeleton.card()
                    ForgeSkeleton.card()
                }
            }
        }
    }
}

private struct P031ArtifactListCard: View {
    let rows: [P031ArtifactSummaryPresentation]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Artifacts")
                .font(.headline)
            if rows.isEmpty {
                ForgeEmptyState(
                    title: "No artifacts",
                    systemImage: "doc",
                    description: "No artifact projections returned."
                )
            } else {
                ForEach(rows, id: \.artifactID) { row in
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text(row.title)
                                .font(.subheadline.weight(.semibold))
                            Spacer()
                            P031FreshnessBadge(state: row.freshnessState)
                        }
                        Text(row.detailLabel)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Label(
                            row.payloadAvailabilityLabel,
                            systemImage: row.payloadAvailabilitySymbolName
                        )
                            .font(.caption)
                        P031CopyItemsView(items: row.diagnosticCopyItems)
                    }
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                }
            }
        }
    }
}

private struct P031ArtifactViewerCard: View {
    let rows: [P031ArtifactViewerPresentation]
    let focusedStageID: String?
    let loadArtifactPreview: (String) async -> P031ArtifactViewerPresentation?
    @State private var selectedArtifactID: String?
    @State private var previewRowsByArtifactID: [String: P031ArtifactViewerPresentation] = [:]
    @State private var loadingPreviewArtifactID: String?
    @State private var artifactSearchText = ""
    @State private var selectedStageID = P031ArtifactViewerCard.allFilterID
    @State private var selectedAgentID = P031ArtifactViewerCard.allFilterID
    @State private var selectedTypeID = P031ArtifactViewerCard.allFilterID
    @State private var selectedGrouping: P031ArtifactGrouping = .iteration
    private let artifactViewerPaneHeight: CGFloat = 620
    private let artifactPreviewTopAnchorID = "p031-artifact-preview-top"
    private static let allFilterID = "__all__"
    private static let unknownAgentID = "__unknown_agent__"

    private func selectedRow(in visibleRows: [P031ArtifactViewerPresentation]) -> P031ArtifactViewerPresentation? {
        if let selectedArtifactID,
           let row = visibleRows.first(where: { $0.artifactID == selectedArtifactID }) {
            return row
        }
        return nil
    }

    private var visibleRows: [P031ArtifactViewerPresentation] {
        rows.filter(matchesFilters)
    }

    private func groupedRows(from visibleRows: [P031ArtifactViewerPresentation]) -> [P031ArtifactGroup] {
        var groups: [P031ArtifactGroup] = []
        var groupIndexByID: [String: Int] = [:]
        for row in visibleRows {
            let group = selectedGrouping.group(for: row)
            if let index = groupIndexByID[group.id] {
                groups[index].rows.append(row)
            } else {
                groupIndexByID[group.id] = groups.count
                groups.append(P031ArtifactGroup(id: group.id, title: group.title, rows: [row]))
            }
        }
        return selectedGrouping.sortedGroups(groups)
    }

    private var stageOptions: [P031ArtifactFilterOption] {
        filterOptions(from: rows.map { (stageFilterID(for: $0), stageTitle(for: $0)) })
    }

    private var agentOptions: [P031ArtifactFilterOption] {
        filterOptions(
            from: rows.map { row in
                let id = agentFilterID(for: row)
                return (id, agentTitle(forFilterID: id))
            }
        )
    }

    private var typeOptions: [P031ArtifactFilterOption] {
        filterOptions(
            from: rows.map { row in
                let kind = P031ArtifactTypeFilter.resolve(row)
                return (kind.rawValue, kind.title)
            }
        )
    }

    private var filtersAreActive: Bool {
        !artifactSearchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            || selectedStageID != Self.allFilterID
            || selectedAgentID != Self.allFilterID
            || selectedTypeID != Self.allFilterID
    }

    var body: some View {
        let visibleRows = visibleRows
        let selectedListRow = selectedRow(in: visibleRows)
        let selectedRow = selectedListRow.flatMap { row in
            previewRowsByArtifactID[row.artifactID] ?? row
        }
        let groupedRows = groupedRows(from: visibleRows)
        let selectedRowID = selectedListRow?.artifactID

        VStack(alignment: .leading, spacing: 12) {
            Text("Artifacts")
                .font(.headline)
            if rows.isEmpty {
                ForgeEmptyState(
                    title: "No artifacts",
                    systemImage: "doc",
                    description: "No artifact projections returned."
                )
            } else {
                VStack(alignment: .leading, spacing: 12) {
                    filterControls(visibleCount: visibleRows.count)

                    HStack(alignment: .top, spacing: 14) {
                        artifactList(
                            visibleRows: visibleRows,
                            groupedRows: groupedRows,
                            selectedRowID: selectedRowID
                        )
                            .frame(width: 340)

                        Divider()

                        artifactPreviewScroll(selectedRow: selectedRow, selectedListRow: selectedListRow)
                            .frame(maxWidth: .infinity, alignment: .topLeading)
                    }
                    .frame(height: artifactViewerPaneHeight)
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
            }
        }
        .onAppear {
            applyFocusedStageIfNeeded(focusedStageID)
            synchronizeSelection(with: visibleRows)
        }
        .onChange(of: focusedStageID) {
            applyFocusedStageIfNeeded(focusedStageID)
            synchronizeSelection(with: visibleRows)
        }
        .onChange(of: visibleRows.map(\.artifactID)) {
            synchronizeSelection(with: visibleRows)
        }
    }

    private func filterControls(visibleCount: Int) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Picker("Stage", selection: $selectedStageID) {
                    Text("All stages").tag(Self.allFilterID)
                    ForEach(stageOptions) { option in
                        Text(option.title).tag(option.id)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("p031-artifact-stage-filter")

                Picker("Agent", selection: $selectedAgentID) {
                    Text("All agents").tag(Self.allFilterID)
                    ForEach(agentOptions) { option in
                        Text(option.title).tag(option.id)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("p031-artifact-agent-filter")

                Picker("Type", selection: $selectedTypeID) {
                    Text("All types").tag(Self.allFilterID)
                    ForEach(typeOptions) { option in
                        Text(option.title).tag(option.id)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("p031-artifact-type-filter")

                Picker("Group", selection: $selectedGrouping) {
                    ForEach(P031ArtifactGrouping.allCases) { grouping in
                        Text(grouping.title).tag(grouping)
                    }
                }
                .pickerStyle(.segmented)
                .frame(minWidth: 300, maxWidth: 360)
                .accessibilityIdentifier("p031-artifact-grouping-picker")

                Spacer(minLength: 8)

                if filtersAreActive {
                    Button("Reset", systemImage: "xmark.circle") {
                        resetFilters()
                    }
                    .buttonStyle(.borderless)
                    .controlSize(.small)
                }
            }

            HStack(spacing: 8) {
                TextField("Search artifacts", text: $artifactSearchText)
                    .textFieldStyle(.roundedBorder)
                    .accessibilityIdentifier("p031-artifact-filter-search")

                Text("\(visibleCount)/\(rows.count)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func artifactList(
        visibleRows: [P031ArtifactViewerPresentation],
        groupedRows: [P031ArtifactGroup],
        selectedRowID: String?
    ) -> some View {
        ScrollViewReader { proxy in
            List {
                if visibleRows.isEmpty {
                    ForgeEmptyState(
                        title: "No matching artifacts",
                        systemImage: "line.3.horizontal.decrease.circle",
                        description: "Adjust artifact filters or search text."
                    )
                } else {
                    ForEach(groupedRows) { group in
                        artifactGroupSection(
                            group,
                            selectedRowID: selectedRowID,
                            isLatestGroup: selectedGrouping == .iteration
                                && group.id == groupedRows.first?.id
                        )
                    }
                }
            }
            .listStyle(.plain)
            .accessibilityIdentifier("p031-artifact-list-scroll")
            .onChange(of: selectedRowID) {
                guard let newValue = selectedRowID else { return }
                withAnimation(.easeInOut(duration: 0.16)) {
                    proxy.scrollTo(newValue, anchor: .center)
                }
            }
        }
    }

    private func artifactGroupSection(
        _ group: P031ArtifactGroup,
        selectedRowID: String?,
        isLatestGroup: Bool
    ) -> some View {
        Section {
            ForEach(group.rows, id: \.artifactID) { row in
                artifactListRow(for: row, selectedRowID: selectedRowID)
                    .id(row.artifactID)
                    .listRowInsets(EdgeInsets(top: 3, leading: 0, bottom: 3, trailing: 4))
                    .listRowSeparator(.hidden)
            }
        } header: {
            HStack {
                Text(group.title)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if isLatestGroup {
                    Text("Latest")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(ForgeStatusColor.success)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(ForgeStatusColor.success.opacity(0.12), in: Capsule())
                }
                Spacer()
                Text("\(group.rows.count)")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.tertiary)
            }
            .accessibilityIdentifier("p031-artifact-group-section")
        }
    }

    private func artifactListRow(for row: P031ArtifactViewerPresentation, selectedRowID: String?) -> some View {
        let displayRow = displayRow(for: row)
        return Button {
            ForgeLogger.ui.info(
                "P031 artifact selected artifactID=\(row.artifactID) title=\(row.title) payloadState=\(row.payloadState.rawValue) renderMode=\(String(describing: row.renderMode)) hasCachedPreview=\((previewRowsByArtifactID[row.artifactID] != nil)) listReason=\(row.unavailableReason ?? "nil")"
            )
            selectedArtifactID = row.artifactID
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(displayRow.title)
                        .font(.caption.weight(.semibold))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    P031FreshnessBadge(state: row.freshnessState)
                }
                HStack(spacing: 8) {
                    Label(label(for: displayRow), systemImage: symbol(for: displayRow))
                        .font(.caption2.weight(.medium))
                    Text(shortContext(for: displayRow))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                row.artifactID == selectedRowID
                    ? Color.accentColor.opacity(0.12)
                    : Color(nsColor: .controlBackgroundColor),
                in: RoundedRectangle(cornerRadius: 10)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .stroke(
                        row.artifactID == selectedRowID
                            ? Color.accentColor.opacity(0.45)
                            : Color.clear,
                        lineWidth: 1
                    )
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(row.accessibilityLabel)
    }

    private func displayRow(
        for row: P031ArtifactViewerPresentation
    ) -> P031ArtifactViewerPresentation {
        previewRowsByArtifactID[row.artifactID] ?? row
    }

    private func artifactPreviewScroll(
        selectedRow: P031ArtifactViewerPresentation?,
        selectedListRow: P031ArtifactViewerPresentation?
    ) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                Color.clear
                    .frame(height: 0)
                    .id(artifactPreviewTopAnchorID)
                artifactPreview(selectedRow: selectedRow)
                    .padding(.trailing, 6)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .accessibilityIdentifier("p031-artifact-preview-scroll")
            .onChange(of: selectedRow?.artifactID) {
                proxy.scrollTo(artifactPreviewTopAnchorID, anchor: .top)
            }
            .task(id: selectedListRow?.artifactID) {
                await loadPreviewIfNeeded(for: selectedListRow)
            }
        }
    }

    @ViewBuilder
    private func artifactPreview(selectedRow: P031ArtifactViewerPresentation?) -> some View {
        if let selectedRow {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .top, spacing: 12) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(selectedRow.title)
                            .font(.subheadline.weight(.semibold))
                            .lineLimit(2)
                            .truncationMode(.middle)
                        Text(selectedRow.subtitle)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                            .truncationMode(.middle)
                    }
                    Spacer()
                    Label(label(for: selectedRow), systemImage: symbol(for: selectedRow))
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.secondary)
                }
                Divider()
                if loadingPreviewArtifactID == selectedRow.artifactID,
                   selectedRow.preparedPreview == nil {
                    VStack(alignment: .leading, spacing: 12) {
                        ForgeSkeleton.headline(width: 250)
                        ForgeSkeleton.text(width: .infinity)
                        ForgeSkeleton.text(width: .infinity)
                        ForgeSkeleton.text(width: 300)
                    }
                    .frame(maxWidth: .infinity, minHeight: 180, alignment: .topLeading)
                } else if let preparedPreview = selectedRow.preparedPreview,
                   let context = renderContext(for: selectedRow) {
                    ArtifactContentRenderer(preparedPreview: preparedPreview, context: context)
                        .frame(maxWidth: .infinity, minHeight: 180, alignment: .topLeading)
                } else {
                    ForgeEmptyState(
                        title: "Payload unavailable",
                        systemImage: "exclamationmark.triangle",
                        description: selectedRow.unavailableReason ?? "GraphQL did not return renderable artifact content."
                    )
                }
            }
        } else if !rows.isEmpty {
            ForgeEmptyState(
                title: "No artifact selected",
                systemImage: "doc.text.magnifyingglass",
                description: "The first visible artifact is selected automatically when filters match results."
            )
        }
    }

    private func synchronizeSelection(with visibleRows: [P031ArtifactViewerPresentation]) {
        let displayedRows = groupedRows(from: visibleRows).flatMap(\.rows)
        let visibleIDs = Set(visibleRows.map(\.artifactID))
        if let selectedArtifactID, visibleIDs.contains(selectedArtifactID) {
            return
        }
        selectedArtifactID = displayedRows.first?.artifactID
    }

    private func applyFocusedStageIfNeeded(_ stageID: String?) {
        guard let stageID,
              rows.contains(where: { stageFilterID(for: $0) == stageID })
        else { return }
        selectedStageID = stageID
    }

    private func loadPreviewIfNeeded(for row: P031ArtifactViewerPresentation?) async {
        guard let row else {
            ForgeLogger.ui.debug("P031 artifact preview skipped: no selected row")
            return
        }
        guard previewRowsByArtifactID[row.artifactID] == nil else {
            ForgeLogger.ui.debug("P031 artifact preview skipped: cached artifactID=\(row.artifactID)")
            return
        }
        ForgeLogger.ui.info(
            "P031 artifact preview request starting artifactID=\(row.artifactID) listPayloadState=\(row.payloadState.rawValue) listRenderMode=\(String(describing: row.renderMode)) listHasPreview=\((row.preparedPreview != nil)) listReason=\(row.unavailableReason ?? "nil")"
        )
        loadingPreviewArtifactID = row.artifactID
        let preview = await loadArtifactPreview(row.artifactID)
        guard selectedArtifactID == row.artifactID else {
            ForgeLogger.ui.info(
                "P031 artifact preview ignored stale response artifactID=\(row.artifactID) currentSelection=\(selectedArtifactID ?? "nil")"
            )
            if loadingPreviewArtifactID == row.artifactID {
                loadingPreviewArtifactID = nil
            }
            return
        }
        if let preview {
            ForgeLogger.ui.info(
                "P031 artifact preview received artifactID=\(row.artifactID) payloadState=\(preview.payloadState.rawValue) renderMode=\(String(describing: preview.renderMode)) hasPreview=\((preview.preparedPreview != nil)) previewChars=\(preview.preparedPreview?.content.count ?? 0) reason=\(preview.unavailableReason ?? "nil")"
            )
            previewRowsByArtifactID[row.artifactID] = preview
        } else {
            ForgeLogger.ui.error(
                "P031 artifact preview loader returned nil artifactID=\(row.artifactID)"
            )
        }
        if loadingPreviewArtifactID == row.artifactID {
            loadingPreviewArtifactID = nil
        }
    }

    private func matchesFilters(_ row: P031ArtifactViewerPresentation) -> Bool {
        if selectedStageID != Self.allFilterID, stageFilterID(for: row) != selectedStageID {
            return false
        }
        if selectedAgentID != Self.allFilterID, agentFilterID(for: row) != selectedAgentID {
            return false
        }
        if selectedTypeID != Self.allFilterID,
           P031ArtifactTypeFilter.resolve(row).rawValue != selectedTypeID {
            return false
        }

        let query = artifactSearchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return true }
        return searchableText(for: row).localizedCaseInsensitiveContains(query)
    }

    private func searchableText(for row: P031ArtifactViewerPresentation) -> String {
        [
            row.title,
            row.subtitle,
            row.stageID,
            row.stageLabel,
            row.iteration.map { "Iteration \($0)" },
            row.attemptNumber.map { "Attempt \($0)" },
            row.agentID,
            row.contractID,
            row.format,
            row.unavailableReason,
        ]
        .compactMap { $0 }
        .joined(separator: " ")
    }

    private func filterOptions(from pairs: [(String, String)]) -> [P031ArtifactFilterOption] {
        var titleByID: [String: String] = [:]
        for pair in pairs where !pair.0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            titleByID[pair.0, default: pair.1] = pair.1
        }
        return titleByID
            .map { P031ArtifactFilterOption(id: $0.key, title: $0.value) }
            .sorted {
                $0.title.localizedStandardCompare($1.title) == .orderedAscending
            }
    }

    private func agentFilterID(for row: P031ArtifactViewerPresentation) -> String {
        let agent = row.agentID?.trimmingCharacters(in: .whitespacesAndNewlines)
        return agent?.isEmpty == false ? agent! : Self.unknownAgentID
    }

    private func agentTitle(forFilterID id: String) -> String {
        id == Self.unknownAgentID ? "Unknown agent" : id
    }

    private func stageTitle(for row: P031ArtifactViewerPresentation) -> String {
        let label = row.stageLabel?.trimmingCharacters(in: .whitespacesAndNewlines)
        return label?.isEmpty == false ? label! : "Stage \(row.stageID)"
    }

    private func stageFilterID(for row: P031ArtifactViewerPresentation) -> String {
        row.stageExecutionID ?? row.stageID
    }

    private func shortContext(for row: P031ArtifactViewerPresentation) -> String {
        let labels = [
            row.iteration.map { "Iter \($0)" },
            row.attemptNumber.map { "att \($0)" },
            row.agentID,
        ]
        .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
        .filter { !$0.isEmpty }
        return labels.isEmpty ? row.subtitle : labels.joined(separator: " / ")
    }

    private func resetFilters() {
        artifactSearchText = ""
        selectedStageID = Self.allFilterID
        selectedAgentID = Self.allFilterID
        selectedTypeID = Self.allFilterID
    }

    private func renderContext(for row: P031ArtifactViewerPresentation) -> ArtifactRenderContext? {
        switch row.renderMode {
        case .markdown:
            return ArtifactRenderContext.explicitNamed(format: .markdown, artifactName: row.title)
        case .json:
            return ArtifactRenderContext.explicitNamed(format: .json, artifactName: row.title)
        case .diff:
            return ArtifactRenderContext.explicitNamed(format: .diff, artifactName: row.title)
        case .plainText:
            return ArtifactRenderContext.explicitNamed(format: .report, artifactName: row.title)
        case .metadataOnly, .unavailable:
            return nil
        }
    }

    private func label(for row: P031ArtifactViewerPresentation) -> String {
        switch row.renderMode {
        case .markdown:
            return "Markdown"
        case .json:
            return "JSON"
        case .diff:
            return "Diff"
        case .plainText:
            return "Text"
        case .metadataOnly:
            return "Metadata"
        case .unavailable:
            return row.payloadState == .available ? "Open to preview" : "No preview"
        }
    }

    private func symbol(for row: P031ArtifactViewerPresentation) -> String {
        switch row.renderMode {
        case .markdown:
            return "doc.richtext"
        case .json:
            return "curlybraces"
        case .diff:
            return "plusminus"
        case .plainText:
            return "doc.text"
        case .metadataOnly:
            return "info.circle"
        case .unavailable:
            return row.payloadState == .available
                ? "doc.text.magnifyingglass"
                : "exclamationmark.triangle"
        }
    }
}

private struct P031ArtifactFilterOption: Identifiable, Equatable {
    let id: String
    let title: String
}

private struct P031ArtifactGroup: Identifiable, Equatable {
    let id: String
    let title: String
    var rows: [P031ArtifactViewerPresentation]
}

private enum P031ArtifactGrouping: String, CaseIterable, Identifiable {
    case iteration
    case stage
    case agent
    case type

    var id: String { rawValue }

    var title: String {
        switch self {
        case .iteration:
            return "Iteration"
        case .stage:
            return "Stage"
        case .agent:
            return "Agent"
        case .type:
            return "Type"
        }
    }

    func group(for row: P031ArtifactViewerPresentation) -> P031ArtifactGroup {
        switch self {
        case .iteration:
            return P031ArtifactGroup(
                id: iterationGroupID(for: row),
                title: iterationGroupTitle(for: row),
                rows: []
            )
        case .stage:
            let label = row.stageLabel?.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = label?.isEmpty == false ? label! : "Stage \(row.stageID)"
            return P031ArtifactGroup(id: "stage:\(row.stageID)", title: title, rows: [])
        case .agent:
            let agent = row.agentID?.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = agent?.isEmpty == false ? agent! : "Unknown agent"
            return P031ArtifactGroup(id: "agent:\(title)", title: title, rows: [])
        case .type:
            let kind = P031ArtifactTypeFilter.resolve(row)
            return P031ArtifactGroup(id: "type:\(kind.rawValue)", title: kind.title, rows: [])
        }
    }

    func sortedGroups(_ groups: [P031ArtifactGroup]) -> [P031ArtifactGroup] {
        guard self == .iteration else {
            return groups
        }
        return groups.sorted { lhs, rhs in
            let lhsKey = iterationSortKey(for: lhs)
            let rhsKey = iterationSortKey(for: rhs)
            if lhsKey.iteration != rhsKey.iteration {
                return lhsKey.iteration < rhsKey.iteration
            }
            if lhsKey.attempt != rhsKey.attempt {
                return lhsKey.attempt < rhsKey.attempt
            }
            return lhsKey.title.localizedStandardCompare(rhsKey.title) == .orderedAscending
        }
    }

    private func iterationGroupID(for row: P031ArtifactViewerPresentation) -> String {
        guard let iteration = row.iteration else {
            return "iteration:unknown"
        }
        let attempt = row.attemptNumber.map(String.init) ?? "unknown"
        return "iteration:\(iteration):attempt:\(attempt)"
    }

    private func iterationGroupTitle(for row: P031ArtifactViewerPresentation) -> String {
        guard let iteration = row.iteration else {
            return "Unknown iteration"
        }
        if let attempt = row.attemptNumber {
            return "Iteration \(iteration), attempt \(attempt)"
        }
        return "Iteration \(iteration)"
    }

    private func iterationSortKey(for group: P031ArtifactGroup) -> (
        iteration: Int,
        attempt: Int,
        title: String
    ) {
        guard let row = group.rows.first, let iteration = row.iteration else {
            return (Int.max, Int.max, group.title)
        }
        return (-iteration, -(row.attemptNumber ?? 0), group.title)
    }
}

private enum P031ArtifactTypeFilter: String, CaseIterable, Identifiable {
    case summary
    case review
    case report
    case diff
    case diagnostic
    case receipt
    case transcript
    case release
    case delivery
    case test
    case markdown
    case json
    case text
    case metadata
    case unavailable
    case other

    var id: String { rawValue }

    var title: String {
        switch self {
        case .summary:
            return "Summary"
        case .review:
            return "Review"
        case .report:
            return "Report"
        case .diff:
            return "Diff"
        case .diagnostic:
            return "Diagnostic"
        case .receipt:
            return "Receipt"
        case .transcript:
            return "Transcript"
        case .release:
            return "Release"
        case .delivery:
            return "Delivery"
        case .test:
            return "Test"
        case .markdown:
            return "Markdown"
        case .json:
            return "JSON"
        case .text:
            return "Text"
        case .metadata:
            return "Metadata"
        case .unavailable:
            return "Unavailable"
        case .other:
            return "Other"
        }
    }

    static func resolve(_ row: P031ArtifactViewerPresentation) -> P031ArtifactTypeFilter {
        let haystack = [
            row.title,
            row.contractID,
            row.subtitle,
            row.format,
        ]
        .joined(separator: " ")
        .lowercased()

        if haystack.contains("summary") {
            return .summary
        }
        if haystack.contains("review") {
            return .review
        }
        if haystack.contains("report") {
            return .report
        }
        if haystack.contains("diff") || haystack.contains("patch") || row.renderMode == .diff {
            return .diff
        }
        if haystack.contains("diagnostic") || haystack.contains("trace")
            || haystack.contains("debug") || haystack.contains("log") {
            return .diagnostic
        }
        if haystack.contains("receipt") {
            return .receipt
        }
        if haystack.contains("transcript") {
            return .transcript
        }
        if haystack.contains("release") || haystack.contains("manifest") {
            return .release
        }
        if haystack.contains("delivery") || haystack.contains("publish")
            || haystack.contains("upload") {
            return .delivery
        }
        if haystack.contains("test") {
            return .test
        }

        switch row.renderMode {
        case .markdown:
            return .markdown
        case .json:
            return .json
        case .diff:
            return .diff
        case .plainText:
            return .text
        case .metadataOnly:
            return .metadata
        case .unavailable:
            return .unavailable
        }
    }
}

private struct P031CatalogContextCard: View {
    let presentation: P031CatalogContextPresentation?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Catalog")
                .font(.headline)
            if let presentation {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(presentation.workflowTitle)
                            .font(.subheadline.weight(.semibold))
                        Spacer()
                        Text(presentation.statusText)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    P031CopyItemsView(
                        items: [
                            presentation.workflowID.map {
                                P031DiagnosticCopyItem(label: "workflow_id", value: $0)
                            },
                            presentation.workflowSnapshotHash.map {
                                P031DiagnosticCopyItem(label: "workflow_snapshot_hash", value: $0)
                            },
                            presentation.catalogSnapshotHash.map {
                                P031DiagnosticCopyItem(label: "catalog_snapshot_hash", value: $0)
                            },
                        ].compactMap { $0 }
                    )
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                .accessibilityLabel(presentation.accessibilityLabel)
            } else {
                ForgeEmptyState(
                    title: "Catalog unavailable",
                    systemImage: "book",
                    description: "The selected run did not include GraphQL-readable workflow catalog metadata."
                )
            }
        }
    }
}

private struct P031ReportMetadataCard: View {
    let rows: [P031ReportMetadataRowPresentation]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Reports")
                .font(.headline)
            if rows.isEmpty {
                ForgeEmptyState(
                    title: "No reports",
                    systemImage: "terminal",
                    description: "No report metadata projections returned."
                )
            } else {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(alignment: .center, spacing: 12) {
                            Text(row.title)
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer(minLength: 8)
                            Label(row.availabilityLabel, systemImage: row.availabilitySymbolName)
                                .font(.caption.weight(.medium))
                                .foregroundStyle(row.canOpenPayload ? .primary : .secondary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                                .frame(width: CGFloat(row.payloadIndicatorSlotWidth), alignment: .trailing)
                        }
                        P031CopyItemsView(items: row.copyItems)
                    }
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                    .accessibilityLabel(row.accessibilityLabel)
                }
            }
        }
    }
}

private struct P031DaemonLifecycleCard: View {
    let presentation: P031DaemonLifecyclePresentation?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Daemon lifecycle")
                    .font(.headline)
                Spacer()
                if let presentation {
                    P031FreshnessBadge(snapshot: presentation.freshness)
                }
            }
            if let presentation {
                P031CalloutCard(
                    title: presentation.title,
                    bodyText: presentation.detailLabel ?? presentation.refreshFeedbackText,
                    accentColor: daemonAccentColor(for: presentation.state)
                ) {
                    VStack(alignment: .leading, spacing: 10) {
                        P031BadgeRow(labels: presentation.badgeLabels)
                        P031CopyItemsView(items: presentation.copyItems)
                        if let errorDescription = presentation.errorDescription {
                            Text(errorDescription)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    ForgeSkeleton.headline(width: 200)
                    ForgeSkeleton.text(width: 150)
                }
                .padding()
            }
        }
        .accessibilityIdentifier("p031-daemon-lifecycle-card")
    }

    private func daemonAccentColor(for state: P031DaemonLifecycleState?) -> Color {
        switch state {
        case .ready:
            return .green
        case .degraded:
            return .orange
        case .failed:
            return .red
        case .restarting, .starting, .notStarted, .shutdown, nil:
            return .secondary
        }
    }
}

private struct P031CopyItemsView: View {
    let items: [P031DiagnosticCopyItem]

    var body: some View {
        if items.isEmpty {
            EmptyView()
        } else {
            FlowLayout(spacing: 8) {
                ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                    Button {
                        copyToPasteboard(item.value)
                    } label: {
                        Label(item.label, systemImage: "doc.on.doc")
                            .font(.caption)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
        }
    }

    private func copyToPasteboard(_ value: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
        #endif
    }
}

private struct P031BadgeRow: View {
    let labels: [String]

    var body: some View {
        FlowLayout(spacing: 6) {
            ForEach(labels, id: \.self) { label in
                Text(label)
                    .font(.caption2.weight(.medium))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.accentColor.opacity(0.12), in: Capsule())
            }
        }
    }
}

private struct P031FreshnessBadge: View {
    let label: String
    let tint: Color

    init(snapshot: P031FreshnessSnapshot) {
        self.init(state: snapshot.state)
    }

    init(state: P031FreshnessState) {
        switch state {
        case .live:
            label = "Live"
            tint = .green
        case .refreshing:
            label = "Refreshing"
            tint = .blue
        case .projectionLag:
            label = "Projection lag"
            tint = .orange
        case .stale:
            label = "Stale"
            tint = .yellow
        case .unavailable:
            label = "Unavailable"
            tint = .red
        case .unauthorized:
            label = "Unauthorized"
            tint = .secondary
        }
    }

    var body: some View {
        Text(label)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(tint.opacity(0.14), in: Capsule())
            .foregroundStyle(tint)
            .accessibilityIdentifier("p031-freshness-\(label.lowercased().replacingOccurrences(of: " ", with: "-"))")
    }
}



// P081 Defect1: Generate a UUIDv7 string for approval action idempotency keys.
// UUIDv7 embeds the current Unix timestamp in ms in the high 48 bits, version nibble 0x7
// in bits 48-51, and random bytes elsewhere (RFC 9562). The server validates
// idempotency keys as UUIDv7 (version nibble check) so UUIDv4 from UUID() must not be used.
private nonisolated func makeUUIDv7() -> String {
    let nowMs = UInt64(Date().timeIntervalSince1970 * 1000)
    var bytes = [UInt8](repeating: 0, count: 16)
    bytes[0] = UInt8((nowMs >> 40) & 0xFF)
    bytes[1] = UInt8((nowMs >> 32) & 0xFF)
    bytes[2] = UInt8((nowMs >> 24) & 0xFF)
    bytes[3] = UInt8((nowMs >> 16) & 0xFF)
    bytes[4] = UInt8((nowMs >> 8) & 0xFF)
    bytes[5] = UInt8(nowMs & 0xFF)
    for i in 6..<16 { bytes[i] = UInt8.random(in: 0...255) }
    bytes[6] = (bytes[6] & 0x0F) | 0x70  // version = 7
    bytes[8] = (bytes[8] & 0x3F) | 0x80  // variant = 10xx
    return String(
        format: "%02X%02X%02X%02X-%02X%02X-%02X%02X-%02X%02X-%02X%02X%02X%02X%02X%02X",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

#Preview("Stages") {
    P036RunsHomePreviewHost(initialTab: .stages)
        .frame(width: 1200, height: 780)
}

#Preview("Artifacts") {
    P036RunsHomePreviewHost(initialTab: .artifacts)
        .frame(width: 1200, height: 780)
}

#Preview("Overview") {
    P036RunsHomePreviewHost(initialTab: .overview)
        .frame(width: 1200, height: 780)
}

#Preview("Timeline") {
    P036RunsHomePreviewHost(initialTab: .timeline)
        .frame(width: 1200, height: 780)
}

private struct P036RunsHomePreviewHost: View {
    let initialTab: P031RunDetailTab

    @StateObject private var model = P031ThinReadDashboardModel.previewLoaded()
    @StateObject private var workbench = RunsWorkbenchPresentationModel()

    var body: some View {
        RunsHomeView(model: model, workbench: workbench, initialTab: initialTab)
            .onAppear {
                if let runsHome = model.runsHome {
                    workbench.populate(from: runsHome)
                }
                if let runDetail = model.runDetail {
                    workbench.populate(from: runDetail)
                }
                workbench.populate(daemon: model.daemonLifecycle, scheduler: model.schedulerHealth)
            }
    }
}

#Preview("Timeline Card") {
    P036TimelineWorkbenchCard(entries: [
        RunsWorkbenchPresentationModel.TimelineEntry(
            id: "agent-session",
            kind: .sessionEvent,
            title: "Code Writer",
            detail: "Session started",
            timestamp: Date(),
            displayTime: "08:00",
            stageID: "implementation_refined",
            surfaceLabel: "session_event",
            agentID: "code-writer",
            sessionID: "session-active",
            isCollapsed: false
        ),
        RunsWorkbenchPresentationModel.TimelineEntry(
            id: "agent-tool",
            kind: .mergedTool,
            title: "Code Writer",
            detail: "Tool: edit (running)",
            timestamp: Date(),
            displayTime: "08:01",
            stageID: "implementation_refined",
            surfaceLabel: "merged_tool",
            agentID: "code-writer",
            sessionID: "session-active",
            isCollapsed: false
        ),
        RunsWorkbenchPresentationModel.TimelineEntry(
            id: "completed-agent-noise",
            kind: .text,
            title: "Previous Agent",
            detail: "Collapsed after completion",
            timestamp: Date(),
            displayTime: "07:58",
            stageID: "implementation_reviewed",
            surfaceLabel: "text",
            agentID: "previous-agent",
            sessionID: "session-complete",
            isCollapsed: true
        )
    ], activeAgents: [], resolveTimelineRawDetail: { _ in
        P031TimelineRawDetailReadModel(
            status: .missing,
            rawDetail: nil,
            rawDetailBytes: nil,
            rawDetailDigest: nil,
            errorReason: .handleNotFound
        )
    })
    .frame(width: 760)
    .padding()
}

private let p093TimelinePromptPreviewDetail = """
## System Instructions
Review the proposal as a macOS specialist. Focus on native macOS interaction patterns, SwiftUI/AppKit fit, accessibility, windowing, menus, keyboard workflows, and platform-specific UX risk. Output only the proposal_review_v1 contract.
---
## Task: dynamic_review_proposal_reviewer_macos
Run meta-root (absolute): /Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218
Workspace root: /Users/user/Documents/Chainworks Forge

### Input Artifacts
- `idea_brief` -> `/Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/context/idea.md`
- `proposal_current` -> `/Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/proposals/current/proposal.md`
- `proposal_feedback_coverage` -> `/Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/reviews/proposal/feedback-coverage.json`
- `reviewer_scope_plan` -> `/Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/reviews/proposal/reviewer-scope-plan.json`
- `score_lift_backlog` -> `/Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/reviews/proposal/score-lift-backlog.json`

### Required Outputs
Return each required output through the final `CHAINWORKS_OUTPUT` object using the canonical path keys below; the engine will materialize canonical files after contract validation.
Tool stdout is not an output channel. Only the final assistant message is settled for `CHAINWORKS_OUTPUT`. Do not call shell `echo`, `printf`, or file-writing commands to return `CHAINWORKS_OUTPUT`.
- `proposal_review_macos` -> `/Users/user/Documents/Chainworks Forge/.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/reviews/proposal/macos.json`

### Structured Output Requirements
CRITICAL: Each required output file must contain exactly one top-level JSON object and nothing else.
- Do NOT wrap the JSON in code fences.
- Do NOT emit markdown, prose, or companion files unless they are explicitly listed as required outputs.
- Every listed field below MUST be present in the JSON, with its correct type.
"""

#Preview("Timeline Prompt Detail") {
    P093FormattedTimelineDetail(
        result: P093TimelineFormattedResult.render(detail: p093TimelinePromptPreviewDetail)
    )
    .frame(width: 1200, height: 420)
    .padding()
}

#if DEBUG
struct P093TimelineProofSurface: View {
    var singleAgentOnly = false

    var body: some View {
        ZStack(alignment: .topLeading) {
            ScrollView {
                P036TimelineWorkbenchCard(
                    entries: proofEntries,
                    activeAgents: proofAgents,
                    resolveTimelineRawDetail: { _ in
                        P031TimelineRawDetailReadModel(
                            status: .available,
                            rawDetail: p093TimelinePromptPreviewDetail,
                            rawDetailBytes: p093TimelinePromptPreviewDetail.utf8.count,
                            rawDetailDigest: "p093-proof-detail",
                            errorReason: nil
                        )
                    }
                )
                .padding(24)
            }
            .frame(minWidth: 920, minHeight: 620)

            Color.clear
                .frame(width: 1, height: 1)
                .accessibilityIdentifier("ui-test-direct-surface-ready-p093_timeline_proof")
        }
    }

    private var proofAgents: [RunsWorkbenchPresentationModel.ActiveTimelineAgent] {
        let agents = [
            RunsWorkbenchPresentationModel.ActiveTimelineAgent(
                id: "code_writer",
                title: "Code Writer",
                providerID: "codex",
                stageID: "state_10_implementation_refined",
                stageLabel: "Implementation refined",
                taskLabel: "refine_implementation",
                status: "running",
                sessionID: "session-p093-code-writer",
                latestAt: Date(timeIntervalSince1970: 1_778_000_000),
                eventCount: 2,
                selectionOrder: 0,
                selectionUnavailableReason: nil
            ),
            RunsWorkbenchPresentationModel.ActiveTimelineAgent(
                id: "reviewer",
                title: "Reviewer",
                providerID: "claude",
                stageID: "state_9_implementation_reviewed",
                stageLabel: "Implementation reviewed",
                taskLabel: "review_implementation",
                status: "running",
                sessionID: "session-p093-reviewer",
                latestAt: Date(timeIntervalSince1970: 1_777_999_900),
                eventCount: 1,
                selectionOrder: 1,
                selectionUnavailableReason: nil
            ),
        ]
        return singleAgentOnly ? Array(agents.prefix(1)) : agents
    }

    private var proofEntries: [RunsWorkbenchPresentationModel.TimelineEntry] {
        let codeWriterEntries = [
            RunsWorkbenchPresentationModel.TimelineEntry(
                id: "rte_p093_prompt",
                kind: .text,
                title: "Prompt sent",
                detail: p093TimelinePromptPreviewDetail,
                timestamp: Date(timeIntervalSince1970: 1_778_000_010),
                displayTime: "10:00:10",
                stageID: "state_10_implementation_refined",
                surfaceLabel: "operator_prompt",
                agentID: "code_writer",
                sessionID: "session-p093-code-writer",
                isCollapsed: false,
                rawDetail: p093TimelinePromptPreviewDetail,
                rawDetailBytes: p093TimelinePromptPreviewDetail.utf8.count,
                rawDetailTruncated: false,
                rawDetailHandle: nil,
                rawDetailDigest: "p093-proof-detail",
                fullRawAvailable: true,
                detailDigest: "p093-proof-detail",
                detailCharCount: p093TimelinePromptPreviewDetail.count,
                chunkCount: 1,
                isStreaming: false,
                isTerminal: true,
                stateLabel: "state_10_implementation_refined",
                providerID: "codex"
            ),
            RunsWorkbenchPresentationModel.TimelineEntry(
                id: "rte_p093_tool",
                kind: .mergedTool,
                title: "Provider activity",
                detail: "completed · rg -n \"TimelineEntryRow\" \"Chainworks Forge/Views/RunsHomeView.swift\"",
                timestamp: Date(timeIntervalSince1970: 1_778_000_015),
                displayTime: "10:00:15",
                stageID: "state_10_implementation_refined",
                surfaceLabel: "provider_activity",
                agentID: "code_writer",
                sessionID: "session-p093-code-writer",
                isCollapsed: false,
                isStreaming: false,
                isTerminal: true,
                stateLabel: "state_10_implementation_refined",
                providerID: "codex"
            ),
            RunsWorkbenchPresentationModel.TimelineEntry(
                id: "rte_p093_response",
                kind: .text,
                title: "Agent response complete",
                detail: "Completed response summary: proposal review output ready.",
                timestamp: Date(timeIntervalSince1970: 1_778_000_020),
                displayTime: "10:00:20",
                stageID: "state_10_implementation_refined",
                surfaceLabel: "agent_summary",
                agentID: "code_writer",
                sessionID: "session-p093-code-writer",
                isCollapsed: false,
                rawDetail: """
                Streaming response body with `inline code` and a short list.

                - first
                - second

                ```json
                {
                  "contract": "proposal_review_v1",
                  "verdict": "ready"
                }
                ```
                """,
                rawDetailBytes: """
                Streaming response body with `inline code` and a short list.

                - first
                - second

                ```json
                {
                  "contract": "proposal_review_v1",
                  "verdict": "ready"
                }
                ```
                """.utf8.count,
                rawDetailTruncated: false,
                rawDetailHandle: nil,
                rawDetailDigest: "p093-proof-response-detail",
                fullRawAvailable: true,
                detailDigest: "p093-proof-response-detail",
                detailCharCount: 164,
                chunkCount: 4,
                isStreaming: false,
                isTerminal: true,
                stateLabel: "state_10_implementation_refined",
                providerID: "codex"
            ),
        ]
        guard !singleAgentOnly else { return codeWriterEntries }
        return codeWriterEntries + [
            RunsWorkbenchPresentationModel.TimelineEntry(
                id: "rte_p093_reviewer",
                kind: .text,
                title: "Reviewer note",
                detail: "Reviewer-only timeline entry.",
                timestamp: Date(timeIntervalSince1970: 1_778_000_030),
                displayTime: "10:00:30",
                stageID: "state_9_implementation_reviewed",
                surfaceLabel: "text_chunk",
                agentID: "reviewer",
                sessionID: "session-p093-reviewer",
                isCollapsed: false,
                isStreaming: true,
                isTerminal: false,
                stateLabel: "state_9_implementation_reviewed",
                providerID: "claude"
            ),
        ]
    }
}
#endif

private struct P036SystemReadinessCard: View {
    let health: RunsWorkbenchPresentationModel.FreshnessHealth

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("System readiness")
                    .font(ForgeTypography.sectionHeader)
                Spacer()
                if health.isReadinessDeferred {
                    Label("Readiness pending", systemImage: "clock.badge.questionmark")
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("readiness-deferred")
                } else if health.isSystemReady {
                    Label("Ready", systemImage: "checkmark.circle.fill")
                        .foregroundStyle(ForgeStatusColor.success)
                } else {
                    Label("Action required", systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(ForgeStatusColor.warning)
                }
            }

            Divider()

            Grid(alignment: .leading, horizontalSpacing: 20, verticalSpacing: 10) {
                GridRow {
                    Text("Daemon")
                    Text(health.daemonHealth)
                        .foregroundStyle(.secondary)
                }
                GridRow {
                    Text("MCP Hub")
                    Text(health.mcpHubStatus)
                        .foregroundStyle(.secondary)
                }
                GridRow {
                    Text("Capabilities")
                    Text(health.capabilitiesStatus)
                        .foregroundStyle(.secondary)
                }
                if let scheduler = health.schedulerHealth {
                    GridRow {
                        Text("Scheduler")
                        Text(scheduler)
                            .foregroundStyle(.secondary)
                    }
                }
                GridRow {
                    Text("Freshness")
                    Text(health.freshness)
                        .foregroundStyle(.secondary)
                }
            }
            .font(.subheadline)
        }
        .padding(20)
        .background(Color.primary.opacity(0.03))
        .cornerRadius(12)
    }
}

private struct P036RunDetailSummaryCard: View {
    let header: RunsWorkbenchPresentationModel.SummaryHeader
    let onCheckSystemReadiness: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(header.title)
                        .font(.title2.weight(.bold))
                    Text(header.status)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                Spacer()

                Button(action: onCheckSystemReadiness) {
                        Label("Check readiness", systemImage: "checkmark.shield")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
        }
        .padding(20)
        .background(Color.primary.opacity(0.03))
        .cornerRadius(12)
    }
}

private struct P036StageMapCard: View {
    let map: RunsWorkbenchPresentationModel.StageMap

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForgeSectionHeader(
                title: "Stages",
                subtitle: "Frozen workflow snapshot · \(map.stages.count) stage\(map.stages.count == 1 ? "" : "s")",
                symbol: "point.topleft.down.curvedto.point.bottomright.up"
            )

            if map.stages.isEmpty {
                Text("No topology readback yet")
                    .font(ForgeTypography.body)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .frame(maxWidth: .infinity, minHeight: 92, alignment: .center)
                    .background(ForgeColor.Surface.muted, in: RoundedRectangle(cornerRadius: 8))
            } else {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(alignment: .top, spacing: 10) {
                        ForEach(Array(map.layoutColumns.enumerated()), id: \.element.id) { index, column in
                            P036StageTopologyColumnView(column: column)
                            if index < map.connectorColumns.count {
                                P036StageTopologyConnectorColumnView(
                                    column: map.connectorColumns[index]
                                )
                            }
                        }
                    }
                    .padding(.vertical, 2)
                    .padding(.trailing, 4)
                }
            }
        }
        .forgePanel()
    }
}

private enum P036StageTopologyMetrics {
    static let cardWidth: CGFloat = 292
    static let cardHeight: CGFloat = 210
    static let cardGap: CGFloat = 12
    static let columnHeaderHeight: CGFloat = 22
    static let connectorWidth: CGFloat = 34

    static func slotHeight(units: Int) -> CGFloat {
        let safeUnits = max(1, units)
        return CGFloat(safeUnits) * cardHeight + CGFloat(safeUnits - 1) * cardGap
    }
}

private struct P036StageTopologyColumnView: View {
    let column: RunsWorkbenchPresentationModel.StageTopologyColumn

    var body: some View {
        VStack(alignment: .leading, spacing: P036StageTopologyMetrics.cardGap) {
            Text(column.title.uppercased())
                .font(ForgeTypography.micro.weight(.semibold))
                .foregroundStyle(ForgeColor.Text.tertiary)
                .lineLimit(1)
                .frame(
                    width: P036StageTopologyMetrics.cardWidth,
                    height: P036StageTopologyMetrics.columnHeaderHeight,
                    alignment: .leading
                )

            ForEach(column.slots) { slot in
                switch slot.kind {
                case .stage:
                    if let stage = slot.stage {
                        P036StageTopologyCard(stage: stage, heightUnits: slot.heightUnits)
                    }
                case .bridge:
                    P036StageTopologyBridgeLane(
                        label: slot.bridgeLabel ?? "Transition",
                        heightUnits: slot.heightUnits
                    )
                }
            }
        }
    }
}

private struct P036StageTopologyConnectorColumnView: View {
    let column: RunsWorkbenchPresentationModel.StageTopologyConnectorColumn

    var body: some View {
        VStack(spacing: P036StageTopologyMetrics.cardGap) {
            Color.clear
                .frame(
                    width: P036StageTopologyMetrics.connectorWidth,
                    height: P036StageTopologyMetrics.columnHeaderHeight
                )

            ForEach(column.connectors) { connector in
                P036StageTopologyConnectorView(style: connector.style)
                    .frame(
                        width: P036StageTopologyMetrics.connectorWidth,
                        height: P036StageTopologyMetrics.cardHeight
                    )
            }
        }
        .accessibilityHidden(true)
    }
}

private struct P036StageTopologyConnectorView: View {
    let style: RunsWorkbenchPresentationModel.StageTopologyConnector.Style

    var body: some View {
        Group {
            switch style {
            case .primary:
                connector(symbol: "arrow.right", tint: ForgeStatusColor.running)
            case .retry:
                connector(symbol: "arrow.uturn.left", tint: ForgeStatusColor.error)
            case .manual:
                connector(symbol: "arrow.right", tint: ForgeStatusColor.warning)
            case .hidden:
                Color.clear
            }
        }
    }

    private func connector(symbol: String, tint: Color) -> some View {
        Image(systemName: symbol)
            .font(.system(size: 18, weight: .semibold))
            .foregroundStyle(tint)
            .frame(width: 30, height: 30)
            .background(tint.opacity(0.12), in: Circle())
            .overlay {
                Circle().strokeBorder(tint.opacity(0.45), lineWidth: 1)
            }
    }
}

private struct P036StageTopologyBridgeLane: View {
    let label: String
    let heightUnits: Int

    var body: some View {
        VStack(spacing: 8) {
            Spacer()
            Image(systemName: "arrow.right")
                .font(.system(size: 22, weight: .semibold))
                .foregroundStyle(ForgeStatusColor.running)
                .frame(width: 42, height: 42)
                .background(ForgeStatusColor.running.opacity(0.12), in: Circle())
                .overlay {
                    Circle().strokeBorder(ForgeStatusColor.running.opacity(0.4), lineWidth: 1)
                }
            Text(label.uppercased())
                .font(ForgeTypography.micro.weight(.semibold))
                .foregroundStyle(ForgeColor.Text.tertiary)
                .lineLimit(1)
            Spacer()
        }
        .frame(
            width: P036StageTopologyMetrics.cardWidth,
            height: P036StageTopologyMetrics.slotHeight(units: heightUnits)
        )
        .background(ForgeColor.Surface.elevated.opacity(0.45), in: RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
                .foregroundStyle(ForgeColor.Surface.border.opacity(0.7))
        }
        .accessibilityLabel("\(label) transition lane")
    }
}

private struct P036StageTopologyCard: View {
    let stage: RunsWorkbenchPresentationModel.StageCard
    let heightUnits: Int

    init(stage: RunsWorkbenchPresentationModel.StageCard, heightUnits: Int = 1) {
        self.stage = stage
        self.heightUnits = heightUnits
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 8) {
                Text(String(format: "%02d", stage.ordinal))
                    .font(ForgeTypography.micro.monospacedDigit())
                    .foregroundStyle(ForgeColor.Text.tertiary)
                    .frame(width: 24, alignment: .leading)

                VStack(alignment: .leading, spacing: 3) {
                    Text(stage.title)
                        .font(ForgeTypography.cardTitle)
                        .foregroundStyle(ForgeColor.Text.primary)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(stage.ownerAgentTitle)
                        .font(ForgeTypography.micro)
                        .foregroundStyle(ForgeColor.Text.secondary)
                        .lineLimit(1)
                }

                Spacer(minLength: 8)

                StatusCapsule(
                    text: statusLabel(for: stage.status),
                    color: stageColor(for: stage.status),
                    icon: statusIconName(for: stage.status),
                    size: .small
                )
            }

            HStack(spacing: 6) {
                ForEach(stageMetadataChips, id: \.self) { label in
                    Text(label)
                        .font(ForgeTypography.micro)
                        .foregroundStyle(ForgeColor.Text.secondary)
                        .lineLimit(1)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(ForgeColor.Surface.muted, in: Capsule())
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                ForEach(stage.occurrences) { occurrence in
                    P036StageOccurrenceRow(occurrence: occurrence)
                }
                if stage.hiddenOccurrenceCount > 0 {
                    Text("+ \(stage.hiddenOccurrenceCount) more")
                        .font(ForgeTypography.micro)
                        .foregroundStyle(ForgeColor.Text.tertiary)
                }
            }
            .frame(minHeight: 46, alignment: .topLeading)

            if !stage.transitions.isEmpty {
                VStack(alignment: .leading, spacing: 3) {
                    ForEach(stage.transitions.prefix(2)) { transition in
                        HStack(spacing: 5) {
                            Image(systemName: "arrow.right")
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(ForgeColor.Text.tertiary)
                            Text(transition.toLabel)
                                .font(ForgeTypography.micro)
                                .foregroundStyle(ForgeColor.Text.secondary)
                                .lineLimit(1)
                        }
                    }
                }
            }
        }
        .frame(
            width: P036StageTopologyMetrics.cardWidth,
            height: P036StageTopologyMetrics.slotHeight(units: heightUnits),
            alignment: .topLeading
        )
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(stage.isCurrent ? ForgeStatusColor.running.opacity(0.08) : ForgeColor.Surface.elevated)
        )
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(
                    stage.isCurrent ? ForgeStatusColor.running : ForgeColor.Surface.border,
                    lineWidth: stage.isCurrent ? 2 : 1
                )
        }
        .modifier(P036RunningPulse(isActive: stage.status == "active" && stage.isCurrent))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(stage.ordinal). \(stage.title), \(stage.ownerAgentTitle), \(statusLabel(for: stage.status)), \(stageMetadata)")
    }

    private var stageMetadata: String {
        let parts = stageMetadataChips + stage.evidenceLabels
        return parts.isEmpty ? "No stage evidence yet" : parts.joined(separator: " · ")
    }

    private var stageMetadataChips: [String] {
        [
            stage.iterationText,
            stage.attemptText,
            stage.approvalRequired ? "Approval" : nil,
            stage.artifactCount > 0 ? "\(stage.artifactCount) artifacts" : nil
        ].compactMap { $0 }.filter { !$0.isEmpty }
    }

    private func stageColor(for status: String) -> Color {
        switch status {
        case "terminal": return ForgeStatusColor.success
        case "active": return ForgeStatusColor.running
        case "blocked": return ForgeStatusColor.error
        case "pending": return ForgeStatusColor.neutral
        default: return ForgeStatusColor.neutral
        }
    }

    private func statusLabel(for status: String) -> String {
        switch status {
        case "terminal": return "Completed"
        case "active": return "Running"
        case "blocked": return "Blocked"
        case "pending": return "Pending"
        default: return "Unavailable"
        }
    }

    private func statusIconName(for status: String) -> String {
        switch status {
        case "terminal": return "checkmark.circle.fill"
        case "active": return "play.circle.fill"
        case "blocked": return "exclamationmark.octagon.fill"
        case "pending": return "clock.fill"
        default: return "questionmark.circle.fill"
        }
    }
}

private struct P036StageOccurrenceRow: View {
    let occurrence: RunsWorkbenchPresentationModel.StageOccurrence

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(ForgeColor.Text.tertiary)
            VStack(alignment: .leading, spacing: 1) {
                Text(occurrence.agentTitle)
                    .font(ForgeTypography.micro.weight(.semibold))
                    .foregroundStyle(ForgeColor.Text.primary)
                    .lineLimit(1)
                Text(detailText)
                    .font(ForgeTypography.micro)
                    .foregroundStyle(ForgeColor.Text.tertiary)
                    .lineLimit(1)
            }
        }
    }

    private var detailText: String {
        [
            occurrence.taskName,
            occurrence.statusText,
            occurrence.executionCountLabel,
            occurrence.providerLabel.isEmpty ? nil : occurrence.providerLabel
        ].compactMap { $0 }.joined(separator: " · ")
    }
}

private struct P036RunningPulse: ViewModifier {
    let isActive: Bool

    func body(content: Content) -> some View {
        if isActive {
            content
                .shadow(color: ForgeStatusColor.running.opacity(0.35), radius: 5)
        } else {
            content
        }
    }
}

#if os(macOS)
private struct P093StableTimelineScrollView<Content: View>: NSViewRepresentable {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.drawsBackground = false
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.contentView.postsBoundsChangedNotifications = true

        let hostingView = NSHostingView(rootView: rootView(width: 1))
        hostingView.autoresizingMask = [.width]
        hostingView.translatesAutoresizingMaskIntoConstraints = true
        scrollView.documentView = hostingView
        context.coordinator.hostingView = hostingView
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let hostingView = context.coordinator.hostingView else { return }

        let clipView = scrollView.contentView
        let previousOrigin = clipView.bounds.origin
        let previousDocumentHeight = scrollView.documentView?.bounds.height ?? 0
        let previousMaxY = max(0, previousDocumentHeight - clipView.bounds.height)
        let wasNearBottom = previousMaxY - previousOrigin.y <= 20
        let viewportWidth = max(clipView.bounds.width, 1)

        NSAnimationContext.runAnimationGroup { animationContext in
            animationContext.duration = 0
            animationContext.allowsImplicitAnimation = false

            hostingView.rootView = rootView(width: viewportWidth)
            hostingView.layoutSubtreeIfNeeded()

            let fittingHeight = hostingView.fittingSize.height
            let documentHeight = max(fittingHeight, clipView.bounds.height)
            hostingView.frame = NSRect(x: 0, y: 0, width: viewportWidth, height: documentHeight)
            scrollView.documentView = hostingView

            let nextMaxY = max(0, documentHeight - clipView.bounds.height)
            let targetY = wasNearBottom ? nextMaxY : min(previousOrigin.y, nextMaxY)
            clipView.scroll(to: NSPoint(x: 0, y: max(0, targetY)))
            scrollView.reflectScrolledClipView(clipView)
        }
    }

    private func rootView(width: CGFloat) -> AnyView {
        AnyView(
            content
                .frame(width: width, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        )
    }

    final class Coordinator {
        var hostingView: NSHostingView<AnyView>?
    }
}
#endif

private struct P036TimelineWorkbenchCard: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let entries: [RunsWorkbenchPresentationModel.TimelineEntry]
    let activeAgents: [RunsWorkbenchPresentationModel.ActiveTimelineAgent]
    let resolveTimelineRawDetail: (String) async -> P031TimelineRawDetailReadModel

    @State private var expandedEntryID: String?
    @State private var selectedAgentID: String?
    @State private var formatterCache = P093TimelineFormatterCache()

    private var allVisibleEntries: [RunsWorkbenchPresentationModel.TimelineEntry] {
        entries.filter { !$0.isCollapsed }.sorted { lhs, rhs in
            if lhs.timestamp == rhs.timestamp { return lhs.id > rhs.id }
            return lhs.timestamp > rhs.timestamp
        }
    }

    private var agentOptions: [TimelineAgentOption] {
        activeAgents.map { agent in
            TimelineAgentOption(
                id: agent.id,
                title: agent.title,
                providerID: agent.providerID,
                stageID: agent.stageID,
                stageLabel: agent.stageLabel,
                taskLabel: agent.taskLabel,
                status: agent.status,
                sessionID: agent.sessionID,
                latestAt: agent.latestAt,
                eventCount: agent.eventCount,
                selectionOrder: agent.selectionOrder,
                selectionUnavailableReason: agent.selectionUnavailableReason
            )
        }
    }

    private var resolvedSelectedAgentID: String? {
        if let selectedAgentID,
           agentOptions.contains(where: { $0.id == selectedAgentID }) {
            return selectedAgentID
        }
        return agentOptions.first?.id
    }

    private var visibleEntries: [RunsWorkbenchPresentationModel.TimelineEntry] {
        guard let agentID = resolvedSelectedAgentID else { return [] }
        return allVisibleEntries.filter { $0.agentID == agentID }
    }

    private var activeAgentReadbackUnavailable: Bool {
        activeAgents.isEmpty && !allVisibleEntries.isEmpty
    }

    var body: some View {
        ScrollViewReader { proxy in
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Timeline")
                        .font(.title3.weight(.semibold))
                    Text(timelineSubtitle)
                        .font(.caption)
                        .foregroundStyle(ForgeColor.Text.secondary)
                }

                if activeAgentReadbackUnavailable {
                    ContentUnavailableView(
                        "Active-agent selector unavailable",
                        systemImage: "person.crop.circle.badge.exclamationmark",
                        description: Text("Control-plane active-agent readback is unavailable; Timeline will resume when daemon selector data is present.")
                    )
                    .frame(maxWidth: .infinity, minHeight: 160)
                } else if visibleEntries.isEmpty {
                    ContentUnavailableView(
                        "No Timeline Data",
                        systemImage: "waveform.path.ecg",
                        description: Text("No active control-plane timeline events for the selected agent yet.")
                    )
                    .frame(maxWidth: .infinity, minHeight: 160)
                } else {
                    GroupBox("Timeline") {
                        VStack(alignment: .leading, spacing: 10) {
                            if agentOptions.count > 1 {
                                TimelineAgentSelector(
                                    options: agentOptions,
                                    selectedAgentID: resolvedSelectedAgentID,
                                    onSelect: { agentID in
                                        selectedAgentID = agentID
                                        expandedEntryID = nil
                                    }
                                )
                            }

                            ForEach(Array(visibleEntries.enumerated()), id: \.element.id) { index, entry in
                                TimelineEntryRow(
                                    entry: entry,
                                    displayOrder: index,
                                    isExpanded: expandedEntryID == entry.id,
                                    resolveTimelineRawDetail: resolveTimelineRawDetail,
                                    formatterCache: formatterCache,
                                    onToggleExpanded: {
                                        expandedEntryID = expandedEntryID == entry.id ? nil : entry.id
                                    }
                                )
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .animation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82), value: visibleEntries.map(\.id))
                }

                Color.clear
                    .frame(height: 1)
                    .id("live-timeline-bottom")
            }
            .onChange(of: visibleEntries.count) {
                scrollToNewestIfNotInspecting(proxy)
            }
            .onChange(of: visibleEntries.first?.id) {
                scrollToNewestIfNotInspecting(proxy)
            }
        }
        .forgePanel()
        .accessibilityIdentifier("p036-timeline-workbench-card")
    }

    private var timelineSubtitle: String {
        if visibleEntries.isEmpty {
            return "Focused run-detail timeline from control-plane active-agent readback."
        }
        return "\(visibleEntries.count) focused event\(visibleEntries.count == 1 ? "" : "s") from the selected active agent."
    }

    private func scrollToNewestIfNotInspecting(_ proxy: ScrollViewProxy) {
        guard expandedEntryID == nil else { return }
        withAnimation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82)) {
            proxy.scrollTo(visibleEntries.first?.id ?? "live-timeline-bottom", anchor: .top)
        }
    }
}

private struct TimelineAgentOption: Identifiable, Equatable {
    let id: String
    let title: String
    let providerID: String?
    let stageID: String?
    let stageLabel: String?
    let taskLabel: String?
    let status: String
    let sessionID: String?
    let latestAt: Date
    let eventCount: Int
    let selectionOrder: Int?
    let selectionUnavailableReason: String?
}

private struct TimelineAgentSelector: View {
    let options: [TimelineAgentOption]
    let selectedAgentID: String?
    let onSelect: (String) -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(options) { option in
                    Button {
                        onSelect(option.id)
                    } label: {
                        VStack(alignment: .leading, spacing: 3) {
                            HStack(spacing: 6) {
                                Text(option.title.isEmpty ? option.id.replacingOccurrences(of: "_", with: " ").capitalized : option.title)
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(ForgeColor.Text.primary)
                                if let providerID = option.providerID, !providerID.isEmpty {
                                    Text(providerID)
                                        .font(.caption2.weight(.medium))
                                        .foregroundStyle(ForgeColor.Text.secondary)
                                }
                            }
                            Text(selectorDetailLabel(for: option))
                                .font(.caption2)
                                .foregroundStyle(ForgeColor.Text.tertiary)
                                .lineLimit(1)
                            HStack(spacing: 6) {
                                Text(selectionStatusLabel(for: option))
                                Text(latestActivityLabel(for: option))
                            }
                            .font(.caption2)
                            .foregroundStyle(ForgeColor.Text.tertiary)
                            .lineLimit(1)
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(
                            ForgeColor.Surface.muted,
                            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .strokeBorder(
                                    option.id == selectedAgentID ? ForgeColor.Brand.accent : Color.clear,
                                    lineWidth: 1
                                )
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(
                        "Timeline agent \(option.id), provider \(option.providerID ?? "unknown"), \(selectorDetailLabel(for: option)), \(selectionStatusLabel(for: option)), \(latestActivityLabel(for: option))"
                    )
                    .accessibilityIdentifier("p093-active-agent-option-\(option.id)")
                }
            }
        }
        .accessibilityIdentifier("p093-active-agent-selector")
    }

    private func selectorDetailLabel(for option: TimelineAgentOption) -> String {
        if let taskLabel = option.taskLabel?.trimmingCharacters(in: .whitespacesAndNewlines), !taskLabel.isEmpty {
            return taskLabel
        }
        if let stageLabel = option.stageLabel?.trimmingCharacters(in: .whitespacesAndNewlines), !stageLabel.isEmpty {
            return stageLabel
        }
        if let sessionSummary = sessionSummaryLabel(for: option) {
            return sessionSummary
        }
        return "\(option.eventCount) event\(option.eventCount == 1 ? "" : "s")"
    }

    private func sessionSummaryLabel(for option: TimelineAgentOption) -> String? {
        guard let sessionID = option.sessionID?.trimmingCharacters(in: .whitespacesAndNewlines), !sessionID.isEmpty else {
            return nil
        }
        return "Session \(shortSessionID(sessionID))"
    }

    private func selectionStatusLabel(for option: TimelineAgentOption) -> String {
        if let reason = option.selectionUnavailableReason?.trimmingCharacters(in: .whitespacesAndNewlines), !reason.isEmpty {
            return "Selector unavailable: \(reason)"
        }
        let status = option.status.trimmingCharacters(in: .whitespacesAndNewlines)
        return status.isEmpty ? "Status unavailable" : status.replacingOccurrences(of: "_", with: " ").capitalized
    }

    private func latestActivityLabel(for option: TimelineAgentOption) -> String {
        let age = max(0, Int(Date().timeIntervalSince(option.latestAt)))
        if age < 60 {
            return "Latest \(age)s ago"
        }
        let minutes = age / 60
        if minutes < 60 {
            return "Latest \(minutes)m ago"
        }
        return "Latest \(minutes / 60)h ago"
    }

    private func shortSessionID(_ id: String) -> String {
        guard id.count > 8 else { return id }
        return String(id.prefix(8))
    }
}

private struct P036ApprovalWorkbenchCard: View {
    let rows: [RunsWorkbenchPresentationModel.ApprovalRow]
    let onApprove: (String) async -> Void
    let onReject: (String) async -> Void
    let resolvingIDs: Set<String>

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Pending approvals")
                .font(ForgeTypography.sectionHeader)

            if rows.isEmpty {
                Text("No pending approvals for this run.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                VStack(spacing: 8) {
                    ForEach(rows) { row in
                        HStack(alignment: .top) {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(row.title)
                                    .font(.subheadline.weight(.medium))
                                if let body = row.body {
                                    Text(body)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                        .lineLimit(3)
                                }
                                // PC-001: follow-up reference (e.g. ticket or issue ID)
                                if let followUpID = row.followUpID {
                                    Label(followUpID, systemImage: "link")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                                if let deferred = row.deferredState {
                                    Text(deferred.displayLabel)
                                        .font(.caption2)
                                    .foregroundStyle(deferred.tint)
                                }
                            }
                            Spacer()

                            HStack(spacing: 8) {
                                // PC-001: diagnostic copy menu when copy items are available
                                if !row.copyItems.isEmpty {
                                    Menu {
                                        ForEach(row.copyItems, id: \.label) { item in
                                            Button(item.label) {
                                                #if os(macOS)
                                                NSPasteboard.general.clearContents()
                                                NSPasteboard.general.setString(item.value, forType: .string)
                                                #endif
                                            }
                                        }
                                    } label: {
                                        Image(systemName: "doc.on.doc")
                                            .imageScale(.small)
                                    }
                                    .menuStyle(.borderlessButton)
                                    .controlSize(.small)
                                    .help("Copy diagnostic details")
                                }

                                Button("Reject") {
                                    Task { await onReject(row.id) }
                                }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                                .tint(ForgeStatusColor.error)
                                .disabled(!row.canReject || resolvingIDs.contains(row.id))
                                .help(row.rejectDisabledReason ?? "")

                                Button("Approve") {
                                    Task { await onApprove(row.id) }
                                }
                                .buttonStyle(.borderedProminent)
                                .controlSize(.small)
                                .tint(ForgeStatusColor.success)
                                .disabled(!row.canApprove || resolvingIDs.contains(row.id))
                                .help(row.approveDisabledReason ?? "")
                            }
                        }
                        .padding(10)
                        .background(Color.primary.opacity(0.04))
                        .cornerRadius(8)
                        .accessibilityLabel(row.accessibilityLabel)
                    }
                }
            }
        }
        .forgePanel(tint: rows.isEmpty ? ForgeColor.Surface.border : ForgeStatusColor.approval)
    }
}

private struct P036ArtifactWorkbenchCard: View {
    let rows: [RunsWorkbenchPresentationModel.ArtifactReportRow]

    private let visibleRowLimit = 24

    private var visibleRows: ArraySlice<RunsWorkbenchPresentationModel.ArtifactReportRow> {
        rows.prefix(visibleRowLimit)
    }

    private var hiddenRowCount: Int {
        max(0, rows.count - visibleRows.count)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForgeSectionHeader(
                title: "Artifacts and reports",
                subtitle: rows.isEmpty ? "No durable outputs yet" : "\(rows.count) durable output\(rows.count == 1 ? "" : "s")",
                symbol: "doc.text"
            )

            if rows.isEmpty {
                Text("No artifacts available.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                VStack(spacing: 8) {
                    ForEach(visibleRows) { row in
                        ArtifactRowView(row: row)
                    }
                    if hiddenRowCount > 0 {
                        Text("\(hiddenRowCount) more durable output\(hiddenRowCount == 1 ? "" : "s") available in artifact detail")
                            .font(ForgeTypography.micro)
                            .foregroundStyle(ForgeColor.Text.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.top, 2)
                    }
                }
            }
        }
        .forgePanel()
    }

    private struct ArtifactRowView: View {
        let row: RunsWorkbenchPresentationModel.ArtifactReportRow

        var body: some View {
            HStack {
                Label(row.title, systemImage: "doc.fill")
                    .font(.subheadline)
                Spacer()
                availabilityBadge(row.payloadAvailability)
            }
            .padding(10)
            .background(Color.primary.opacity(0.04))
            .cornerRadius(8)
        }

        @ViewBuilder
        private func availabilityBadge(_ availability: RunsWorkbenchPresentationModel.ArtifactPayloadAvailability) -> some View {
            let color = availabilityColor(availability)
            Text(availability.rawValue.capitalized)
                .font(.caption2)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(color.opacity(0.15))
                .foregroundStyle(color)
                .clipShape(Capsule())
        }

        private func availabilityColor(_ availability: RunsWorkbenchPresentationModel.ArtifactPayloadAvailability) -> Color {
            switch availability {
            case .available: return ForgeStatusColor.success
            case .metadataOnly: return ForgeStatusColor.running
            case .generating: return ForgeStatusColor.warning
            case .deferred: return .secondary
            case .unavailable, .unknown: return ForgeStatusColor.error
            }
        }
    }
}

private struct P036RecoveryEvidenceCard: View {
    let rows: [RunsWorkbenchPresentationModel.RecoveryEvidenceRow]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForgeSectionHeader(
                title: "Recovery evidence",
                subtitle: rows.isEmpty ? "No recovery facts yet" : "\(rows.count) diagnostic row\(rows.count == 1 ? "" : "s")",
                symbol: "bandage"
            )

            if rows.isEmpty {
                Text("No recovery evidence found.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(rows) { row in
                        Label(row.title, systemImage: "bandage.fill")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .forgePanel()
    }
}

// MARK: - P046 Session Observability Card

// Renders P046 session observability readback in the selected-run overview tab.
// Shows lineage list, KPI summary, health warnings, and generic MCP reset guidance.
// Hides itself when P046 is unavailable (feature flag off or schema absent).
private struct P046SessionObservabilityCard: View {
    @ObservedObject var model: P046SessionObservabilityModel

    var body: some View {
        switch model.availability {
        case .unavailable:
            EmptyView()
        case .unknown:
            if model.isLoading {
                loadingShell
            } else {
                EmptyView()
            }
        case .available:
            contentCard
        }
    }

    private var loadingShell: some View {
        HStack(spacing: 10) {
            ProgressView().controlSize(.small)
            Text("Loading session observability…")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.03))
        .cornerRadius(12)
        .accessibilityIdentifier("p046-session-observability-loading")
    }

    private var contentCard: some View {
        VStack(alignment: .leading, spacing: 16) {
            headerRow
            if let health = model.health {
                healthSection(health)
            }
            if !model.lineages.isEmpty {
                lineagesSection
            }
            if let kpi = model.kpiSummary {
                kpiRow(kpi)
            }
            if shouldShowResetGuidance {
                Text("Suggested MCP action: use the MCP session reset capability.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("p046-mcp-reset-guidance")
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.03))
        .cornerRadius(12)
        .accessibilityIdentifier("p046-session-observability-card")
    }

    private var shouldShowResetGuidance: Bool {
        guard let state = model.health?.state else { return false }
        return state == "WARNING" || state == "CRITICAL"
    }

    private var headerRow: some View {
        HStack {
            Text("Session Observability")
                .font(.headline)
            if model.isStale {
                Label("Stale", systemImage: "arrow.clockwise")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }
            Spacer()
            if let kpi = model.kpiSummary {
                Text("\(kpi.lineageCount) lineage\(kpi.lineageCount == 1 ? "" : "s")")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private func healthSection(_ health: P046SessionHealthReadModel) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Label(healthLabel(health.state), systemImage: healthIcon(health.state))
                .font(.subheadline.weight(.medium))
                .foregroundStyle(healthColor(health.state))
            if !health.warnings.isEmpty {
                ForEach(health.warnings, id: \.reasonCode) { warning in
                    HStack(alignment: .top, spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .foregroundStyle(severityColor(warning.severity))
                        VStack(alignment: .leading, spacing: 2) {
                            Text(warning.reasonCode.replacingOccurrences(of: "_", with: " ").capitalized)
                                .font(.caption.weight(.medium))
                            if let message = warning.message {
                                Text(message)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
            }
        }
    }

    private var lineagesSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Agent Sessions")
                .font(.subheadline.weight(.medium))
            ForEach(model.lineages) { lineage in
                HStack(spacing: 8) {
                    Circle()
                        .fill(lineageColor(lineage.healthState))
                        .frame(width: 7, height: 7)
                    VStack(alignment: .leading, spacing: 1) {
                        HStack(spacing: 4) {
                            Text(lineage.agentId)
                                .font(.caption.weight(.medium))
                            if let scope = lineage.sessionReuseScope {
                                Text("·")
                                    .font(.caption)
                                    .foregroundStyle(.tertiary)
                                Text(scope)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        if let count = lineage.generationCount {
                            Text("\(count) generation\(count == 1 ? "" : "s")")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    Spacer()
                    if lineage.activeGenerationId != nil {
                        Text("active")
                            .font(.caption2)
                            .padding(.horizontal, 4)
                            .padding(.vertical, 2)
                            .background(Color.green.opacity(0.12))
                            .foregroundStyle(.green)
                            .cornerRadius(4)
                    }
                }
                .padding(.vertical, 1)
            }
        }
    }

    private func kpiRow(_ kpi: P046SessionKpiSummaryReadModel) -> some View {
        HStack(spacing: 20) {
            kpiChip("Active", value: "\(kpi.activeGenerationCount)")
            if let closed = kpi.closedGenerationCount {
                kpiChip("Closed", value: "\(closed)")
            }
            if let turns = kpi.totalTurnCount {
                kpiChip("Turns", value: "\(turns)")
            }
            Spacer()
        }
    }

    private func kpiChip(_ label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(value).font(.subheadline.weight(.semibold))
            Text(label).font(.caption2).foregroundStyle(.secondary)
        }
    }

    private func healthLabel(_ state: String) -> String {
        switch state {
        case "HEALTHY": return "Healthy"
        case "WARNING": return "Warning"
        case "CRITICAL": return "Critical"
        default: return "Unknown"
        }
    }

    private func healthIcon(_ state: String) -> String {
        switch state {
        case "HEALTHY": return "checkmark.circle.fill"
        case "WARNING": return "exclamationmark.triangle"
        case "CRITICAL": return "xmark.circle.fill"
        default: return "questionmark.circle"
        }
    }

    private func healthColor(_ state: String) -> Color {
        switch state {
        case "HEALTHY": return .green
        case "WARNING": return .orange
        case "CRITICAL": return .red
        default: return .secondary
        }
    }

    private func severityColor(_ severity: String) -> Color {
        switch severity {
        case "INFO": return .blue
        case "WARNING": return .orange
        case "CRITICAL": return .red
        default: return .secondary
        }
    }

    private func lineageColor(_ healthState: String?) -> Color {
        switch healthState {
        case "HEALTHY": return .green
        case "WARNING": return .orange
        case "CRITICAL": return .red
        default: return Color(nsColor: .secondaryLabelColor)
        }
    }
}

/// Simple flow layout for stage cards
private struct FlowLayout: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 0
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var maxHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if currentX + size.width > width && currentX > 0 {
                currentX = 0
                currentY += maxHeight + spacing
                maxHeight = 0
            }
            currentX += size.width + spacing
            maxHeight = max(maxHeight, size.height)
        }

        return CGSize(width: width, height: currentY + maxHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var currentX: CGFloat = bounds.minX
        var currentY: CGFloat = bounds.minY
        var maxHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if currentX + size.width > bounds.maxX && currentX > bounds.minX {
                currentX = bounds.minX
                currentY += maxHeight + spacing
                maxHeight = 0
            }
            subview.place(at: CGPoint(x: currentX, y: currentY), proposal: .unspecified)
            currentX += size.width + spacing
            maxHeight = max(maxHeight, size.height)
        }
    }
}
