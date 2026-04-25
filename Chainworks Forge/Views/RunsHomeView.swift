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
                if let runDetail = model.runDetail {
                    P031RunDetailSummaryCard(presentation: runDetail)
                    P031StageListCard(rows: runDetail.stageRows)
                    P031ApprovalInboxCard(presentation: model.approvalInbox)
                    P031ArtifactListCard(rows: runDetail.artifactRows)
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
    @Published private(set) var selectedRunID: String?

    private let writePathGuideReference: String?
    private let loadRunsHomeAction: @Sendable (P031FreshnessSnapshot, Bool) async -> P031RunsHomePresentation
    private let loadRunDetailAction: @Sendable (String, P031FreshnessSnapshot) async -> P031RunDetailPresentation
    private let loadApprovalInboxAction: @Sendable (P031FreshnessSnapshot) async -> P031ApprovalInboxPresentation
    private let loadDaemonLifecycleAction: @Sendable (P031FreshnessSnapshot) async -> P031DaemonLifecyclePresentation

    private var didLoad = false
    private var orientationDismissed = false
    private var runsFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var runDetailFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var approvalFreshness = P031FreshnessSnapshot(state: .refreshing)
    private var daemonFreshness = P031FreshnessSnapshot(state: .refreshing)

    var canCopyWritePathGuideReference: Bool {
        writePathGuideReference != nil
    }

    init<Store: P031WorkflowReadStore>(
        coordinator: P031ThinWorkflowScreenCoordinator<Store>,
        writePathGuideReference: String? = nil
    ) {
        self.writePathGuideReference = writePathGuideReference
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
        loadApprovalInboxAction = { currentFreshness in
            await coordinator.loadApprovalInbox(currentFreshness: currentFreshness)
        }
        loadDaemonLifecycleAction = { currentFreshness in
            await coordinator.loadDaemonLifecycle(currentFreshness: currentFreshness)
        }
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
        } else if let firstRunID = availableRunIDs.first {
            selectedRunID = firstRunID
            await loadRunDetail(for: firstRunID)
        } else {
            selectedRunID = nil
            runDetail = nil
        }
    }

    func selectRun(_ runID: String) {
        guard selectedRunID != runID else { return }
        selectedRunID = runID
        Task { await loadRunDetail(for: runID) }
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

    private func loadRunDetail(for runID: String) async {
        let presentation = await loadRunDetailAction(runID, runDetailFreshness)
        runDetailFreshness = presentation.freshness
        runDetail = presentation
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
