import SwiftUI
import Combine
#if os(macOS)
import AppKit
#endif

struct RunsHomeView: View {
    @StateObject private var model: P031ThinReadDashboardModel
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
        _model = StateObject(wrappedValue: model)
        self.workbench = workbench
        _selectedRunDetailTab = State(initialValue: .overview)
    }

    init(
        model: P031ThinReadDashboardModel,
        workbench: RunsWorkbenchPresentationModel,
        initialTab: P031RunDetailTab
    ) {
        _model = StateObject(wrappedValue: model)
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

                        if !workbench.artifactsAndReports.isEmpty {
                            P036ArtifactWorkbenchCard(rows: workbench.artifactsAndReports)
                        }

                        if !workbench.recoveryEvidence.isEmpty {
                            P036RecoveryEvidenceCard(rows: workbench.recoveryEvidence)
                        }

                        if let health = workbench.freshnessAndHealth {
                            P036SystemReadinessCard(health: health)
                        }
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
                        P036TimelineWorkbenchCard(entries: timelineEntriesForSelectedRun())
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
                isCollapsed: false
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
    private static let completedResponseDetailLimit = 48_000

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
            append(event)
            return
        }

        let previous = events.remove(at: existingIndex)
        append(P031RuntimeTimelineEventPresentation(
            id: previous.id,
            runID: event.runID,
            stageID: event.stageID ?? previous.stageID,
            agentID: event.agentID,
            provider: event.provider,
            eventKind: event.eventKind,
            title: event.title,
            detail: Self.boundedLiveResponseDetail(previous.detail + event.detail),
            surfaceLabel: event.surfaceLabel,
            sessionGenerationID: previous.sessionGenerationID ?? event.sessionGenerationID,
            timestamp: event.timestamp
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
        let summaryDetail = Self.summaryDetail(
            for: matchingChunks,
            override: terminalDetailOverride
        )
        guard !summaryDetail.isEmpty else { return }

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
                timestamp: fallbackTerminalEvent.timestamp
            )
        )
    }

    private static func isResponseChunk(_ event: P031RuntimeTimelineEventPresentation) -> Bool {
        event.surfaceLabel == "agent_message_chunk" || event.surfaceLabel == "text_chunk"
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

    private static func summaryDetail(
        for chunks: [P031RuntimeTimelineEventPresentation],
        override: String?
    ) -> String {
        var detail = chunks
            .map(\.detail)
            .filter { !$0.isEmpty }
            .joined()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if detail.isEmpty {
            detail = override?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        }
        if detail.count > completedResponseDetailLimit {
            detail = String(detail.suffix(completedResponseDetailLimit))
        }
        return detail
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

    static func bootstrap() -> P031ThinReadDashboardModel {
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
                    switch action {
                    case .approve:
                        _ = try await approvalMutationClient.approve(approvalID: approvalID)
                    case .reject(let reason):
                        _ = try await approvalMutationClient.reject(
                            approvalID: approvalID,
                            reason: reason
                        )
                    }
                    return nil
                } catch {
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

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(entry.title)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(ForgeColor.Text.primary)
                    .lineLimit(1)

                if entry.isCollapsed {
                    StatusCapsule(
                        text: "Collapsed",
                        color: ForgeStatusColor.neutral,
                        icon: "rectangle.compress.vertical",
                        size: .small
                    )
                }

                Spacer(minLength: 8)

                Text(entry.surfaceLabel)
                    .font(.caption2)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .lineLimit(1)
            }

            if !entry.detail.isEmpty {
                Text(previewDetail)
                    .font(.caption)
                    .foregroundStyle(ForgeColor.Text.secondary)
                    .lineLimit(previewLineLimit)
                    .fixedSize(horizontal: false, vertical: false)
                    .frame(maxHeight: previewMaxHeight, alignment: .top)
                    .clipped()
            }

            HStack(spacing: 8) {
                if let stageID = entry.stageID, !stageID.isEmpty {
                    Text(stageID)
                }
                if let sessionID = entry.sessionID, !sessionID.isEmpty {
                    Text(sessionID)
                }
                if let displayTime = entry.displayTime {
                    Text(displayTime)
                        .monospacedDigit()
                }
            }
            .font(.caption2)
            .foregroundStyle(ForgeColor.Text.tertiary)
            .lineLimit(1)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(tint.opacity(entry.isCollapsed ? 0.10 : 0.20), lineWidth: 1)
        )
        .transition(reduceMotion ? .opacity : .asymmetric(
            insertion: .push(from: .bottom).combined(with: .opacity),
            removal: .opacity
        ))
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

    private var previewLineLimit: Int {
        isResponseEntry ? 40 : 3
    }

    private var previewMaxHeight: CGFloat {
        isResponseEntry ? 560 : 54
    }

    private var isResponseEntry: Bool {
        entry.surfaceLabel == "text_chunk"
            || entry.surfaceLabel == "agent_message_chunk"
            || entry.surfaceLabel == "agent_summary"
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
    ])
    .frame(width: 760)
    .padding()
}

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
                        ForEach(Array(map.stages.enumerated()), id: \.element.id) { index, stage in
                            if index > 0 {
                                Image(systemName: "chevron.right")
                                    .font(.system(size: 13, weight: .semibold))
                                    .foregroundStyle(ForgeColor.Text.tertiary)
                                    .frame(width: 18, height: 164)
                                    .accessibilityHidden(true)
                            }
                            P036StageTopologyCard(stage: stage)
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

private struct P036StageTopologyCard: View {
    let stage: RunsWorkbenchPresentationModel.StageCard

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
        .frame(width: 272, alignment: .topLeading)
        .frame(minHeight: 164, alignment: .topLeading)
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

private struct P036TimelineWorkbenchCard: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let entries: [RunsWorkbenchPresentationModel.TimelineEntry]

    private var visibleEntries: [RunsWorkbenchPresentationModel.TimelineEntry] {
        entries.filter { !$0.isCollapsed }
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

                if visibleEntries.isEmpty {
                    ContentUnavailableView(
                        "No Timeline Data",
                        systemImage: "waveform.path.ecg",
                        description: Text("No active control-plane timeline events for the selected agent yet.")
                    )
                    .frame(maxWidth: .infinity, minHeight: 160)
                } else {
                    GroupBox("Timeline") {
                        VStack(alignment: .leading, spacing: 10) {
                            ForEach(visibleEntries) { entry in
                                TimelineEntryRow(entry: entry)
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
                withAnimation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82)) {
                    proxy.scrollTo("live-timeline-bottom", anchor: .bottom)
                }
            }
            .onChange(of: visibleEntries.last?.id) {
                withAnimation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82)) {
                    proxy.scrollTo("live-timeline-bottom", anchor: .bottom)
                }
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
