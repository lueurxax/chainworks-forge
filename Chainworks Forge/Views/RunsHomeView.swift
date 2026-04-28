import SwiftUI
import Combine
#if os(macOS)
import AppKit
#endif

struct RunsHomeView: View {
    @StateObject private var model = P031ThinReadDashboardModel.bootstrap()

    var body: some View {
        NavigationSplitView {
            runsSidebar
                .navigationSplitViewColumnWidth(min: 280, ideal: 320)
        } detail: {
            runDetailPane
        }
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
            if let runsHome = model.runsHome {
                if let orientation = runsHome.orientation {
                    Section {
                        P031CalloutCard(
                            title: orientation.title,
                            bodyText: orientation.body,
                            accentColor: .blue
                        ) {
                            HStack(spacing: 12) {
                                Button {
                                    model.copyWritePathGuideReference()
                                } label: {
                                    Label(
                                        orientation.externalWritePathLabel,
                                        systemImage: model.canCopyWritePathGuideReference
                                            ? "doc.on.doc" : "link"
                                    )
                                    .font(.caption)
                                }
                                .buttonStyle(.link)
                                .disabled(!model.canCopyWritePathGuideReference)
                                Spacer()
                                if orientation.canDismiss {
                                    Button("Dismiss") {
                                        Task { await model.dismissOrientation() }
                                    }
                                    .buttonStyle(.borderless)
                                    .font(.caption)
                                }
                            }
                        }
                    }
                }

                Section {
                    if runsHome.rows.isEmpty {
                        P031EmptySectionRow(
                            title: runsHome.emptyStateTitle ?? "No runs",
                            detail: runsHome.errorDescription ?? runsHome.refreshFeedbackText
                        )
                    } else {
                        ForEach(runsHome.rows, id: \.runID) { row in
                            Button {
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
                    }
                } header: {
                    P031SectionHeader(
                        title: "Runs",
                        subtitle: runsHome.refreshFeedbackText,
                        freshness: runsHome.freshness
                    )
                }
            } else {
                Section {
                    ProgressView("Checking latest data")
                        .frame(maxWidth: .infinity, alignment: .leading)
                } header: {
                    Text("Runs")
                }
            }

            Section {
                P031WritePathGuideSummaryView(summary: model.writePathGuideSummary)
            } header: {
                Text("External write paths")
            }
        }
        .listStyle(.sidebar)
    }

    private var runDetailPane: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
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
                }

                if let runDetail = model.runDetail {
                    P031RunDetailSummaryCard(presentation: runDetail)
                    P031IdeaContextCard(presentation: runDetail.ideaContext)
                    P031StageTransitionMapCard(rows: runDetail.stageTransitions)
                    P031ArtifactViewerCard(
                        rows: runDetail.artifactViewerRows,
                        loadArtifactPreview: model.loadArtifactPreview
                    )
                    P031CatalogContextCard(presentation: runDetail.catalogContext)
                    P031ApprovalInboxCard(presentation: model.approvalInbox)
                    P031ReportMetadataCard(rows: runDetail.reportRows)
                } else {
                    P031CalloutCard(
                        title: "Run detail unavailable",
                        bodyText: model.runsHome?.emptyStateTitle ?? "Select a run to inspect server projections.",
                        accentColor: .secondary
                    )
                }

                P031DaemonLifecycleCard(presentation: model.daemonLifecycle)
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

@MainActor
final class P031ThinReadDashboardModel: ObservableObject {
    @Published private(set) var runsHome: P031RunsHomePresentation?
    @Published private(set) var runDetail: P031RunDetailPresentation?
    @Published private(set) var approvalInbox: P031ApprovalInboxPresentation?
    @Published private(set) var daemonLifecycle: P031DaemonLifecyclePresentation?
    @Published private(set) var writePathGuideSummary: P031OperatorWritePathGuideSummaryPresentation
    @Published private(set) var isLoading = false
    @Published private(set) var isRestartingDaemon = false
    @Published private(set) var daemonRestartError: String?
    @Published private(set) var selectedRunID: String?

    private let writePathGuideReference: String?
    private let loadRunsHomeAction: @Sendable (P031FreshnessSnapshot, Bool) async -> P031RunsHomePresentation
    private let loadRunDetailAction: @Sendable (String, P031FreshnessSnapshot) async -> P031RunDetailPresentation
    private let loadArtifactPreviewAction: (String) async -> P031ArtifactViewerPresentation?
    private let loadApprovalInboxAction: @Sendable (P031FreshnessSnapshot) async -> P031ApprovalInboxPresentation
    private let loadDaemonLifecycleAction: @Sendable (P031FreshnessSnapshot) async -> P031DaemonLifecyclePresentation
    private let subscribeRunStatusAction: @Sendable (String, P031FreshnessSnapshot) throws -> AsyncThrowingStream<P031RunStatusSubscriptionPresentation, Error>
    private let restartDaemonAction: @MainActor @Sendable () async -> String?
    private let bundledDaemonBuildSHAAction: @Sendable () -> String?

    private var didLoad = false
    private var orientationDismissed = false
    private var runsFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runDetailFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var approvalFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var daemonFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runStatusSubscriptionTask: Task<Void, Never>?
    private var subscribedRunID: String?

    var canCopyWritePathGuideReference: Bool {
        writePathGuideReference != nil
    }

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
        writePathGuideReference: String? = nil,
        restartDaemonAction: @escaping @MainActor @Sendable () async -> String? = {
            await P031ThinReadDashboardModel.restartPackagedDaemon()
        },
        bundledDaemonBuildSHAAction: @escaping @Sendable () -> String? = {
            P031ThinReadDashboardModel.bundledDaemonBuildSHA()
        }
    ) {
        self.writePathGuideReference = writePathGuideReference
        self.restartDaemonAction = restartDaemonAction
        self.bundledDaemonBuildSHAAction = bundledDaemonBuildSHAAction
        writePathGuideSummary = coordinator.loadOperatorWritePathGuideSummary()
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
        let coordinator = P031ThinWorkflowScreenCoordinator(
            store: store,
            writePathGuideData: guideResource.data
        )
        return P031ThinReadDashboardModel(
            coordinator: coordinator,
            writePathGuideReference: guideResource.url?.path
        )
    }

    func loadIfNeeded() async {
        guard !didLoad else { return }
        didLoad = true
        await refreshAll()
    }

    func refreshAll() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }

        async let runsTask = loadRunsHomeAction(runsFreshness, !orientationDismissed)
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

    func dismissOrientation() async {
        orientationDismissed = true
        let presentation = await loadRunsHomeAction(runsFreshness, false)
        runsFreshness = presentation.freshness
        runsHome = presentation
    }

    func copyWritePathGuideReference() {
#if os(macOS)
        guard let writePathGuideReference else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(writePathGuideReference, forType: .string)
#endif
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

        async let runsTask = loadRunsHomeAction(runsFreshness, !orientationDismissed)
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
              let raw = try? String(contentsOf: url, encoding: .utf8)
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
    }
}

private struct P031RunDetailSummaryCard: View {
    let presentation: P031RunDetailPresentation

    var body: some View {
        P031CalloutCard(
            title: presentation.title,
            bodyText: detailBody,
            accentColor: .accentColor
        ) {
            HStack(spacing: 10) {
                P031FreshnessBadge(snapshot: presentation.freshness)
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
            presentation.progressLabel,
            presentation.pendingApprovalsLabel,
            presentation.refreshFeedbackText,
        ]
        .compactMap { $0 }
        .joined(separator: " • ")
    }
}

private struct P031StageListCard: View {
    let rows: [P031StageSummaryPresentation]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Stages")
                .font(.headline)
            if rows.isEmpty {
                P031EmptySectionRow(title: "No stages", detail: "No stage projections returned.")
            } else {
                ForEach(rows, id: \.stageExecutionID) { row in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text(row.title)
                                .font(.subheadline.weight(.semibold))
                            Spacer()
                            P031FreshnessBadge(state: row.freshnessState)
                        }
                        Text(row.statusLabel)
                            .font(.caption.weight(.medium))
                        if let iterationLabel = row.iterationLabel {
                            Text(iterationLabel)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        if !row.badgeLabels.isEmpty {
                            P031BadgeRow(labels: row.badgeLabels)
                        }
                    }
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                }
            }
        }
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
                                if !row.evidenceLabels.isEmpty {
                                    P031BadgeRow(labels: row.evidenceLabels)
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
                                if let actionLabel = row.actionLabel {
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
    let loadArtifactPreview: (String) async -> P031ArtifactViewerPresentation?
    @State private var selectedArtifactID: String?
    @State private var previewRowsByArtifactID: [String: P031ArtifactViewerPresentation] = [:]
    @State private var loadingPreviewArtifactID: String?
    @State private var artifactSearchText = ""
    @State private var selectedStageID = P031ArtifactViewerCard.allFilterID
    @State private var selectedAgentID = P031ArtifactViewerCard.allFilterID
    @State private var selectedTypeID = P031ArtifactViewerCard.allFilterID
    @State private var selectedGrouping: P031ArtifactGrouping = .stage
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
        return groups
    }

    private var stageOptions: [P031ArtifactFilterOption] {
        filterOptions(from: rows.map { ($0.stageID, "Stage \($0.stageID)") })
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
                            .frame(width: 320)

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
                .frame(maxWidth: 280)
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
                        artifactGroupSection(group, selectedRowID: selectedRowID)
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

    private func artifactGroupSection(_ group: P031ArtifactGroup, selectedRowID: String?) -> some View {
        Section {
            ForEach(group.rows, id: \.artifactID) { row in
                artifactListRow(for: row, selectedRowID: selectedRowID)
                    .id(row.artifactID)
                    .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 4))
                    .listRowSeparator(.hidden)
            }
        } header: {
            HStack {
                Text(group.title)
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                Text("\(group.rows.count)")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(.tertiary)
            }
            .accessibilityIdentifier("p031-artifact-group-section")
        }
    }

    private func artifactListRow(for row: P031ArtifactViewerPresentation, selectedRowID: String?) -> some View {
        Button {
            ForgeLogger.ui.info(
                "P031 artifact selected artifactID=\(row.artifactID) title=\(row.title) payloadState=\(row.payloadState.rawValue) renderMode=\(String(describing: row.renderMode)) hasCachedPreview=\((previewRowsByArtifactID[row.artifactID] != nil)) listReason=\(row.unavailableReason ?? "nil")"
            )
            selectedArtifactID = row.artifactID
        } label: {
            VStack(alignment: .leading, spacing: 5) {
                HStack {
                    Text(row.title)
                        .font(.caption.weight(.semibold))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    P031FreshnessBadge(state: row.freshnessState)
                }
                Text(row.subtitle)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                Label(label(for: row), systemImage: symbol(for: row.renderMode))
                    .font(.caption2.weight(.medium))
            }
            .padding(10)
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
                HStack(alignment: .firstTextBaseline) {
                    Text(selectedRow.title)
                        .font(.subheadline.weight(.semibold))
                    Spacer()
                    Label(label(for: selectedRow), systemImage: symbol(for: selectedRow.renderMode))
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.secondary)
                }
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
                detail: "Select an artifact to load its preview."
            )
        }
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
        if selectedStageID != Self.allFilterID, row.stageID != selectedStageID {
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
            return "Unavailable"
        }
    }

    private func symbol(for mode: P031ArtifactRenderMode) -> String {
        switch mode {
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
            return "exclamationmark.triangle"
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
    case stage
    case agent
    case type

    var id: String { rawValue }

    var title: String {
        switch self {
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
        case .stage:
            return P031ArtifactGroup(id: "stage:\(row.stageID)", title: "Stage \(row.stageID)", rows: [])
        case .agent:
            let agent = row.agentID?.trimmingCharacters(in: .whitespacesAndNewlines)
            let title = agent?.isEmpty == false ? agent! : "Unknown agent"
            return P031ArtifactGroup(id: "agent:\(title)", title: title, rows: [])
        case .type:
            let kind = P031ArtifactTypeFilter.resolve(row)
            return P031ArtifactGroup(id: "type:\(kind.rawValue)", title: kind.title, rows: [])
        }
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

private struct P031WritePathGuideSummaryView: View {
    let summary: P031OperatorWritePathGuideSummaryPresentation

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if summary.rows.isEmpty {
                P031EmptySectionRow(
                    title: summary.emptyStateTitle ?? "External write-path guide unavailable",
                    detail: "Governed UI remains read-only until a machine-readable guide is supplied."
                )
            } else {
                Text("\(summary.availableExternalWorkflowCount) documented • \(summary.pendingOrInvalidCount) pending • \(summary.unavailableCount) unavailable")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                ForEach(summary.rows, id: \.removedControlID) { row in
                    VStack(alignment: .leading, spacing: 6) {
                        Text(row.title)
                            .font(.subheadline.weight(.semibold))
                        Text(row.statusLabel)
                            .font(.caption.weight(.medium))
                        Text(row.workflowLabel)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        if let toolLabel = row.toolLabel {
                            Text(toolLabel)
                                .font(.caption.monospaced())
                        }
                    }
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                }
            }
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

#Preview {
    RunsHomeView()
}
