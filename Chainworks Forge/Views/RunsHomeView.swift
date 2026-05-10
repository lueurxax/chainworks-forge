import SwiftUI
import Combine
#if os(macOS)
import AppKit
#endif

struct RunsHomeView: View {
    @StateObject private var model: P031ThinReadDashboardModel
    @StateObject private var workbench: RunsWorkbenchPresentationModel
    @State private var selectedRunDetailTab: P031RunDetailTab = .overview
    @State private var focusedArtifactStageID: String?
    @State private var closeoutReadinessScrollRequest = 0
    @FocusState private var closeoutReadinessFocus: P077CloseoutReadinessFocus?

    @MainActor
    init() {
        let model = P031ThinReadDashboardModel.bootstrap()
        _model = StateObject(wrappedValue: model)
        _workbench = StateObject(wrappedValue: RunsWorkbenchPresentationModel())
        _selectedRunDetailTab = State(initialValue: .overview)
    }

    init(
        model: P031ThinReadDashboardModel,
        initialTab: P031RunDetailTab
    ) {
        _model = StateObject(wrappedValue: model)
        _workbench = StateObject(wrappedValue: RunsWorkbenchPresentationModel())
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
    }

    private var runsSidebar: some View {
        List {
            if workbench.sidebarLanes.isEmpty {
                Section {
                    if model.isLoading {
                        ProgressView("Checking latest data")
                            .frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        P031EmptySectionRow(
                            title: model.runsHome?.emptyStateTitle ?? "No runs",
                            detail: model.runsHome?.errorDescription ?? model.runsHome?.refreshFeedbackText ?? ""
                        )
                    }
                } header: {
                    Text("Runs")
                }
            } else {
                ForEach(workbench.sidebarLanes) { lane in
                    Section {
                        ForEach(lane.runs, id: \.runID) { row in
                            Button {
                                selectedRunDetailTab = .overview
                                focusedArtifactStageID = nil
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
                    } header: {
                        Text(lane.title)
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .accessibilityIdentifier("runs-home-list")
        .onChange(of: model.runsHome) { _, newValue in
            if let newValue {
                workbench.populate(from: newValue)
            }
        }
        .onChange(of: model.runDetail) { _, newValue in
            if let newValue {
                workbench.populate(from: newValue)
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
    private var inlineApprovalsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(workbench.inlineApprovals) { approval in
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(approval.title)
                            .font(.headline)
                        if let reason = approval.disabledReason {
                            Text(reason)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    
                    Spacer()
                    
                    if approval.isActionable {
                        HStack(spacing: 8) {
                            Button("Approve") {
                                Task {
                                    await model.settleApproval(approval.id, action: .approve)
                                }
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(.green)
                            
                            Button("Reject") {
                                Task {
                                    await model.settleApproval(approval.id, action: .reject(reason: "Rejected from inline view"))
                                }
                            }
                            .buttonStyle(.bordered)
                            .tint(.red)
                        }
                        .disabled(model.resolvingApprovalIDs.contains(approval.id))
                    } else if let state = approval.deferredState {
                        HStack(spacing: 4) {
                            Image(systemName: "exclamationmark.triangle.fill")
                            Text(state.rawValue)
                        }
                        .font(.caption)
                        .foregroundStyle(.orange)
                    }
                }
                .padding(12)
                .background(Color.primary.opacity(0.04))
                .cornerRadius(8)
            }
        }
        .padding(.top, 4)
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
                        P031RunDetailSummaryCard(
                            presentation: runDetail,
                            onCompactCloseoutActivated: activateCloseoutReadinessFromCompactSignal
                        )
                        if !workbench.inlineApprovals.isEmpty {
                            inlineApprovalsSection
                        }
                        if let closeoutReadiness = runDetail.closeoutReadiness {
                            P077CloseoutReadinessCard(
                                presentation: closeoutReadiness,
                                closeoutFocus: $closeoutReadinessFocus,
                                onReturnToCloseoutReadiness: focusCloseoutPrimaryUnblock
                            )
                            .id(P077CloseoutReadinessAnchor.card)
                        }
                        P031IdeaContextCard(presentation: runDetail.ideaContext)
                        P031CatalogContextCard(presentation: runDetail.catalogContext)
                    case .stages:
                        P031StageTransitionMapCard(
                            rows: runDetail.stageTransitions,
                            artifactCountsByStageID: artifactCountsByStageID(for: runDetail),
                            onArtifactsSelected: { stageID in
                                focusedArtifactStageID = stageID
                                selectedRunDetailTab = .artifacts
                            }
                        )
                    case .artifacts:
                        P031ArtifactViewerCard(
                            rows: runDetail.artifactViewerRows,
                            focusedStageID: focusedArtifactStageID,
                            loadArtifactPreview: model.loadArtifactPreview
                        )
                    case .approvals:
                        P031ApprovalInboxCard(
                            presentation: model.approvalInbox,
                            actionError: model.approvalActionError,
                            isResolving: { model.isResolvingApproval($0) },
                            onApprove: { approvalID in
                                Task { await model.settleApproval(approvalID, action: .approve) }
                            },
                            onReject: { approvalID in
                                Task { await model.settleApproval(approvalID, action: .reject(reason: "Rejected from Chainworks Forge UI")) }
                            }
                        )
                    case .reports:
                        P031ReportMetadataCard(rows: runDetail.reportRows)
                    case .system:
                        P031DaemonLifecycleCard(presentation: model.daemonLifecycle)
                    }
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 20)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .onChange(of: closeoutReadinessScrollRequest) { _, _ in
                guard selectedRunDetailTab == .overview else { return }
                withAnimation(.easeInOut(duration: 0.16)) {
                    proxy.scrollTo(P077CloseoutReadinessAnchor.card, anchor: .top)
                }
                closeoutReadinessFocus = .primaryUnblock
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

enum P031RunDetailTab: String, CaseIterable, Identifiable {
    case overview
    case stages
    case artifacts
    case approvals
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
    @Published private(set) var isLoading = false
    @Published private(set) var isRestartingDaemon = false
    @Published private(set) var resolvingApprovalIDs: Set<String> = []
    @Published private(set) var approvalActionError: String?
    @Published private(set) var daemonRestartError: String?
    @Published private(set) var selectedRunID: String?

    var totalPendingApprovalCount: Int {
        approvalInbox?.rows.count ?? 0
    }

    private let loadRunsHomeAction: @Sendable (P031FreshnessSnapshot, Bool) async -> P031RunsHomePresentation
    private let loadRunDetailAction: @Sendable (String, P031FreshnessSnapshot) async -> P031RunDetailPresentation
    private let loadArtifactPreviewAction: (String) async -> P031ArtifactViewerPresentation?
    private let loadApprovalInboxAction: @Sendable (P031FreshnessSnapshot) async -> P031ApprovalInboxPresentation
    private let loadDaemonLifecycleAction: @Sendable (P031FreshnessSnapshot) async -> P031DaemonLifecyclePresentation
    private let subscribeRunStatusAction: @Sendable (String, P031FreshnessSnapshot) throws -> AsyncThrowingStream<P031RunStatusSubscriptionPresentation, Error>
    private let settleApprovalAction: @Sendable (String, P072ApprovalDecisionAction) async -> String?
    private let restartDaemonAction: @MainActor @Sendable () async -> String?
    private let bundledDaemonBuildSHAAction: @Sendable () -> String?

    private var didLoad = false
    private var runsFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runDetailFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var approvalFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var daemonFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runStatusSubscriptionTask: Task<Void, Never>?
    private var subscribedRunID: String?

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
        }
    ) {
        self.settleApprovalAction = settleApprovalAction
        self.restartDaemonAction = restartDaemonAction
        self.bundledDaemonBuildSHAAction = bundledDaemonBuildSHAAction
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
    }

    deinit {
        runStatusSubscriptionTask?.cancel()
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
                    freshnessState: .live,
                    accessibilityLabel: "Proposal review run, running"
                ),
                P031RunsHomeRowPresentation(
                    runID: "preview-run-implementation",
                    title: "Implementation closeout",
                    workflowLabel: "chainworks_implementation",
                    statusLabel: "Completed",
                    progressLabel: "9 stages, 31 artifacts",
                    pendingApprovalsLabel: nil,
                    freshnessState: .live,
                    accessibilityLabel: "Implementation closeout, completed"
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
        settleApprovalAction = { _, _ in nil }
        restartDaemonAction = { nil }
        bundledDaemonBuildSHAAction = { "preview" }

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
            freshness: freshness,
            refreshFeedbackText: "Live projection",
            emptyStateTitle: nil,
            errorDescription: nil
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

        let runsPresentation = await runsTask
        let approvalsPresentation = await approvalsTask
        let daemonPresentation = await daemonTask

        runsFreshness = runsPresentation.freshness
        approvalFreshness = approvalsPresentation.freshness
        daemonFreshness = daemonPresentation.freshness

        runsHome = runsPresentation
        approvalInbox = approvalsPresentation
        daemonLifecycle = daemonPresentation

        let availableRunIDs = runsPresentation.rows.map { $0.runID }
        if let selectedRunID, availableRunIDs.contains(selectedRunID) {
            await loadRunDetail(for: selectedRunID)
            startRunStatusSubscription(for: selectedRunID)
        } else if let firstRunID = availableRunIDs.first {
            selectedRunID = firstRunID
            await loadRunDetail(for: firstRunID)
            startRunStatusSubscription(for: firstRunID)
        } else {
            selectedRunID = nil
            runDetail = nil
            stopRunStatusSubscription()
        }
    }

    func selectRun(_ runID: String) {
        guard selectedRunID != runID else { return }
        selectedRunID = runID
        startRunStatusSubscription(for: runID)
        Task { await loadRunDetail(for: runID) }
    }

    func loadArtifactPreview(artifactID: String) async -> P031ArtifactViewerPresentation? {
        await loadArtifactPreviewAction(artifactID)
    }

    func isResolvingApproval(_ approvalID: String) -> Bool {
        resolvingApprovalIDs.contains(approvalID)
    }

    func settleApproval(_ approvalID: String, action: P072ApprovalDecisionAction) async {
        guard !resolvingApprovalIDs.contains(approvalID) else { return }
        resolvingApprovalIDs.insert(approvalID)
        approvalActionError = nil
        defer { resolvingApprovalIDs.remove(approvalID) }

        if let error = await settleApprovalAction(approvalID, action) {
            approvalActionError = error
            return
        }
        await refreshAll()
    }

    func restartDaemonForSchemaMismatch() async {
        await restartDaemonForUpdateRequired()
    }

    func restartDaemonForUpdateRequired() async {
        guard !isRestartingDaemon else { return }
        isRestartingDaemon = true
        daemonRestartError = nil
        defer { isRestartingDaemon = false }

        if let error = await restartDaemonAction() {
            daemonRestartError = error
            return
        }
        await refreshAll()
    }

    private func loadRunDetail(for runID: String) async {
        let presentation = await loadRunDetailAction(runID, runDetailFreshness)
        runDetailFreshness = presentation.freshness
        runDetail = presentation
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

    private func stopRunStatusSubscription() {
        runStatusSubscriptionTask?.cancel()
        runStatusSubscriptionTask = nil
        subscribedRunID = nil
    }

    private func refreshSelectedRunAfterSubscriptionEvent(runID: String) async {
        guard selectedRunID == runID else { return }

        async let runsTask = loadRunsHomeAction(runsFreshness, false)
        async let approvalsTask = loadApprovalInboxAction(approvalFreshness)
        async let detailTask = loadRunDetailAction(runID, runDetailFreshness)

        let runsPresentation = await runsTask
        let approvalsPresentation = await approvalsTask
        let detailPresentation = await detailTask

        guard selectedRunID == runID else { return }
        runsFreshness = runsPresentation.freshness
        approvalFreshness = approvalsPresentation.freshness
        runDetailFreshness = detailPresentation.freshness
        runsHome = runsPresentation
        approvalInbox = approvalsPresentation
        runDetail = detailPresentation
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

private struct P031RunDetailSummaryCard: View {
    let presentation: P031RunDetailPresentation
    let onCompactCloseoutActivated: () -> Void

    var body: some View {
        P031CalloutCard(
            title: presentation.title,
            bodyText: detailBody,
            accentColor: .accentColor
        ) {
            HStack(spacing: 10) {
                P031RunsHomeAccessibilityMarker(
                    identifier: "p031-run-detail-summary-\(presentation.freshness.state.rawValue)",
                    label: presentation.title
                )
                P031FreshnessBadge(snapshot: presentation.freshness)
                if let closeoutReadiness = presentation.closeoutReadiness {
                    P077CompactSignalCapsule(
                        label: closeoutReadiness.compactSignalLabel,
                        systemImage: "checkmark.seal",
                        accentColor: P077CloseoutReadinessChrome.accentColor(
                            for: closeoutReadiness.visualState
                        ),
                        accessibilityLabel: closeoutReadiness.compactActivationAccessibilityLabel,
                        accessibilityIdentifier: "p077-closeout-readiness-compact-action",
                        action: onCompactCloseoutActivated
                    )
                }
                if let errorDescription = presentation.errorDescription {
                    Text(errorDescription)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var detailBody: String {
        [
            presentation.workflowLabel,
            presentation.statusLabel,
            rolloutDecisionText,
            presentation.progressLabel,
            presentation.pendingApprovalsLabel,
            presentation.refreshFeedbackText,
        ]
        .compactMap { $0 }
        .joined(separator: " • ")
    }

    private var rolloutDecisionText: String? {
        guard let rollout = presentation.rolloutDecisionSummary else { return nil }
        return "Rollout \(rollout.backendDecision)"
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
        .onChange(of: presentation) { _, _ in
            recordCloseoutAnnouncement(sheetOwnsFocus: isDiagnosticsPresented)
        }
        .onChange(of: isDiagnosticsPresented) { _, sheetOwnsFocus in
            recordCloseoutAnnouncement(sheetOwnsFocus: sheetOwnsFocus)
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
            Text("Idea")
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
                P031EmptySectionRow(
                    title: "Idea context unavailable",
                    detail: "The selected run did not include a GraphQL-readable idea reference."
                )
            }
        }
    }
}

private struct P031StageTransitionMapCard: View {
    let rows: [P031StageTransitionPresentation]
    let artifactCountsByStageID: [String: Int]
    let onArtifactsSelected: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Stage transitions")
                .font(.headline)
            if rows.isEmpty {
                P031EmptySectionRow(title: "No transitions", detail: "No stage projections returned.")
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element.stageExecutionID) { index, row in
                        HStack(alignment: .top, spacing: 12) {
                            VStack(spacing: 0) {
                                Circle()
                                    .fill(color(for: row.connectorState))
                                    .frame(width: 12, height: 12)
                                    .overlay(Circle().stroke(.white.opacity(0.85), lineWidth: 1))
                                if index < rows.count - 1 {
                                    Rectangle()
                                        .fill(color(for: row.connectorState).opacity(0.45))
                                        .frame(width: 2, height: 42)
                                }
                            }
                            VStack(alignment: .leading, spacing: 6) {
                                HStack(alignment: .firstTextBaseline) {
                                    Text(row.stageTitle)
                                        .font(.subheadline.weight(.semibold))
                                    Spacer()
                                    Text(row.statusText)
                                        .font(.caption.weight(.medium))
                                        .foregroundStyle(color(for: row.connectorState))
                                }
                                if let attemptText = row.attemptText {
                                    Text(attemptText)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let startedLabel = row.startedLabel {
                                    Text(startedLabel)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let completedLabel = row.completedLabel {
                                    Text(completedLabel)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                if let durationLabel = row.durationLabel {
                                    Text(durationLabel)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                let artifactCount = artifactCountsByStageID[row.stageExecutionID] ?? 0
                                let evidenceLabels = evidenceLabels(for: row, artifactCount: artifactCount)
                                if !evidenceLabels.isEmpty {
                                    P031BadgeRow(labels: evidenceLabels)
                                }
                                if artifactCount > 0 {
                                    Button {
                                        onArtifactsSelected(row.stageExecutionID)
                                    } label: {
                                        HStack(spacing: 5) {
                                            Image(systemName: "doc.text.magnifyingglass")
                                            Text("\(artifactCount) artifact\(artifactCount == 1 ? "" : "s")")
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
                            .padding(.bottom, index < rows.count - 1 ? 16 : 0)
                        }
                        .accessibilityLabel(row.accessibilityLabel)
                    }
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
            }
        }
    }

    private func evidenceLabels(
        for row: P031StageTransitionPresentation,
        artifactCount: Int
    ) -> [String] {
        guard artifactCount > 0 else { return row.evidenceLabels }
        return row.evidenceLabels.filter { $0 != "Artifacts" }
    }

    private func color(for state: P031StageConnectorState) -> Color {
        switch state {
        case .completed:
            return .green
        case .blocked:
            return .red
        case .running:
            return .blue
        case .pending:
            return .orange
        case .unavailable:
            return .secondary
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
                Text(actionError)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
            if let presentation {
                if presentation.rows.isEmpty {
                    P031EmptySectionRow(
                        title: presentation.emptyStateTitle ?? "No pending approvals",
                        detail: presentation.errorDescription ?? presentation.refreshFeedbackText
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
                ProgressView("Updating approvals")
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
                P031EmptySectionRow(title: "No artifacts", detail: "No artifact projections returned.")
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
                P031EmptySectionRow(title: "No artifacts", detail: "No artifact projections returned.")
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
        .onChange(of: focusedStageID) { _, newValue in
            applyFocusedStageIfNeeded(newValue)
            synchronizeSelection(with: visibleRows)
        }
        .onChange(of: visibleRows.map(\.artifactID)) { _, _ in
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
                    P031EmptySectionRow(
                        title: "No matching artifacts",
                        detail: "Adjust artifact filters or search text."
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
            .onChange(of: selectedRowID) { _, newValue in
                guard let newValue else { return }
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
                        .foregroundStyle(.green)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.green.opacity(0.12), in: Capsule())
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
            .onChange(of: selectedRow?.artifactID) { _, _ in
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
                    ProgressView("Loading artifact preview")
                        .frame(maxWidth: .infinity, minHeight: 180, alignment: .topLeading)
                } else if let preparedPreview = selectedRow.preparedPreview,
                   let context = renderContext(for: selectedRow) {
                    ArtifactContentRenderer(preparedPreview: preparedPreview, context: context)
                        .frame(maxWidth: .infinity, minHeight: 180, alignment: .topLeading)
                } else {
                    P031EmptySectionRow(
                        title: "Payload unavailable",
                        detail: selectedRow.unavailableReason
                            ?? "GraphQL did not return renderable artifact content."
                    )
                }
            }
        } else if !rows.isEmpty {
            P031EmptySectionRow(
                title: "No artifact selected",
                detail: "The first visible artifact is selected automatically when filters match results."
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
                P031EmptySectionRow(
                    title: "Catalog unavailable",
                    detail: "The selected run did not include GraphQL-readable workflow catalog metadata."
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
                P031EmptySectionRow(title: "No reports", detail: "No report metadata projections returned.")
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
                ProgressView("Checking daemon status")
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

private struct P031EmptySectionRow: View {
    let title: String
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.subheadline.weight(.semibold))
            Text(detail)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
    }
}

private struct FlowLayout<Content: View>: View {
    let spacing: CGFloat
    @ViewBuilder let content: Content

    init(spacing: CGFloat, @ViewBuilder content: () -> Content) {
        self.spacing = spacing
        self.content = content()
    }

    var body: some View {
        HStack(alignment: .top, spacing: spacing) {
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

#Preview("Stages") {
    RunsHomeView(model: .previewLoaded(), initialTab: .stages)
        .frame(width: 1200, height: 780)
}

#Preview("Artifacts") {
    RunsHomeView(model: .previewLoaded(), initialTab: .artifacts)
        .frame(width: 1200, height: 780)
}

#Preview("Overview") {
    RunsHomeView(model: .previewLoaded(), initialTab: .overview)
        .frame(width: 1200, height: 780)
}
