import Combine
import SwiftUI

extension Notification.Name {
    static let chainworksSelectTab = Notification.Name("chainworks.selectTab")
    static let chainworksOpenRunInRunsHome = Notification.Name("chainworks.openRunInRunsHome")
}

struct ContentView: View {
    @State private var selectedTab: Tab
    @StateObject private var daemonStatus = DaemonStatusViewModel.bootstrap()
    @StateObject private var schedulerHealth = SchedulerHealthViewModel.bootstrap()

    private let forcedInitialTab: Tab?
    private let forcedUISurface: UISurface?

    enum Tab: String, CaseIterable {
        case runsHome = "Runs Home"
        case ideas = "Ideas"
        case approvals = "Approvals"
        case agentCatalog = "Agent Catalog"
        case workflowInspector = "Workflow Inspector"
        case pilotReadiness = "Pilot Readiness"
        case providerSettings = "Settings"
    }

    enum UISurface: String {
        case completedExportHub = "completed_export_hub"
    }

    init() {
        let environment = ProcessInfo.processInfo.environment
        let initialTab = environment["CHAINWORKS_UI_TEST_INITIAL_TAB"]
            .flatMap(Tab.init(rawValue:))
        forcedInitialTab = initialTab
        forcedUISurface = environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]
            .flatMap(UISurface.init(rawValue:))
        _selectedTab = State(initialValue: initialTab ?? .runsHome)
    }

    var body: some View {
        Group {
            if let forcedUISurface {
                directSurfaceView(for: forcedUISurface)
            } else {
                VStack(spacing: 0) {
                    if daemonStatus.shouldDisplayBanner || schedulerHealth.bannerIssue != nil {
                        DaemonLifecycleBanner(
                            viewModel: daemonStatus,
                            schedulerHealthIssue: schedulerHealth.bannerIssue,
                            onOpenSchedulerHealth: { selectedTab = .pilotReadiness }
                        )
                        .padding(.horizontal, 12)
                        .padding(.top, 8)
                    }
                    tabShell
                }
                .task {
                    await daemonStatus.startSnapshotPlusSubscribe()
                }
                .task {
                    await schedulerHealth.refresh()
                }
            }
        }
    }

    private var tabShell: some View {
        TabView(selection: $selectedTab) {
            tabContent(.runsHome) {
                RunsHomeView()
            }
            .tabItem { Label("Runs Home", systemImage: "house") }
            .tag(Tab.runsHome)

            tabContent(.ideas) {
                if P031IdeasCompatibilitySurface.usesUITestFixture {
                    P031IdeasCompatibilitySurface()
                } else {
                    P031DaemonIdeasSurface()
                }
            }
            .tabItem { Label("Ideas", systemImage: "lightbulb") }
            .tag(Tab.ideas)

            tabContent(.approvals) {
                P031ApprovalCompatibilitySurface()
            }
            .tabItem { Label("Approvals", systemImage: "checkmark.seal") }
            .tag(Tab.approvals)

            tabContent(.agentCatalog) {
                AgentCatalogView(
                    catalogURL: exampleFileURL(
                        environmentKey: "CHAINWORKS_AGENT_CATALOG_SOURCE_PATH",
                        bundleName: "agents",
                        repoRelativePath: "examples/agents/agents.yaml"
                    )
                )
            }
            .tabItem { Label("Agent Catalog", systemImage: "person.3") }
            .tag(Tab.agentCatalog)

            tabContent(.workflowInspector) {
                WorkflowInspectorView(
                    workflowURL: exampleFileURL(
                        environmentKey: "CHAINWORKS_WORKFLOW_SOURCE_PATH",
                        bundleName: "workflow",
                        repoRelativePath: "examples/workflows/workflow.yaml"
                    ),
                    compactWorkflowURL: exampleFileURL(
                        environmentKey: nil,
                        bundleName: "proposal-to-release",
                        repoRelativePath: "examples/workflows/proposal-to-release.yaml"
                    ),
                    catalogURL: exampleFileURL(
                        environmentKey: "CHAINWORKS_AGENT_CATALOG_SOURCE_PATH",
                        bundleName: "agents",
                        repoRelativePath: "examples/agents/agents.yaml"
                    )
                )
            }
            .tabItem { Label("Workflow Inspector", systemImage: "flowchart") }
            .tag(Tab.workflowInspector)

            tabContent(.pilotReadiness) {
                P031OperatorPlaceholder(
                    title: "Pilot Readiness",
                    message: "Readiness evidence is tracked in repository evidence and daemon-backed run projections.",
                    identifier: "pilot-readiness-view",
                    titleIdentifier: "pilot-readiness-title"
                )
            }
            .tabItem { Label("Pilot Readiness", systemImage: "checkmark.shield") }
            .tag(Tab.pilotReadiness)

            tabContent(.providerSettings) {
                P031OperatorPlaceholder(
                    title: "Settings",
                    message: "Provider configuration is owned by the control plane and packaged daemon.",
                    identifier: "provider-settings-view",
                    titleIdentifier: "provider-settings-title"
                )
            }
            .tabItem { Label("Settings", systemImage: "slider.horizontal.3") }
            .tag(Tab.providerSettings)
        }
        .task(id: forcedInitialTab?.rawValue ?? "default") {
            guard let forcedInitialTab, selectedTab != forcedInitialTab else { return }
            selectedTab = forcedInitialTab
        }
        .onReceive(NotificationCenter.default.publisher(for: .chainworksSelectTab)) { notification in
            guard
                let rawValue = notification.userInfo?["tab"] as? String,
                let tab = Tab(rawValue: rawValue)
            else { return }
            selectedTab = tab
        }
        .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenRunInRunsHome)) { _ in
            selectedTab = .runsHome
        }
    }

    @ViewBuilder
    private func tabContent<Content: View>(
        _ tab: Tab,
        @ViewBuilder content: () -> Content
    ) -> some View {
        if selectedTab == tab {
            content()
        } else {
            Color.clear.accessibilityHidden(true)
        }
    }

    @ViewBuilder
    private func directSurfaceView(for surface: UISurface) -> some View {
        switch surface {
        case .completedExportHub:
            P031CompletedExportHubCompatibilitySurface()
        }
    }

    private func exampleFileURL(
        environmentKey: String?,
        bundleName: String,
        repoRelativePath: String
    ) -> URL? {
        let configuredPath = environmentKey
            .flatMap { ProcessInfo.processInfo.environment[$0] }?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let configuredURL = configuredPath
            .flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0) }

        return AppConfiguration.preferredExampleURL(
            configuredURL: configuredURL,
            repoRelativePath: repoRelativePath,
            bundledURL: Bundle.main.url(forResource: bundleName, withExtension: "yaml")
        )
    }
}

private struct P031ApprovalCompatibilitySurface: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Approvals")
                .font(.caption)
                .foregroundStyle(.secondary)
            Text("Approval Inbox")
                .font(.title2.weight(.semibold))
            ContentUnavailableView(
                "No Pending Approvals",
                systemImage: "checkmark.seal",
                description: Text("Approval decisions remain available through the daemon-backed approval surface.")
            )
            .accessibilityIdentifier("approval-inbox-empty-state")
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(24)
        .accessibilityIdentifier("approval-inbox-view")
    }
}

private struct P031DaemonIdeasSurface: View {
    @StateObject private var model = P031DaemonIdeasModel.bootstrap()

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 14) {
                P031AccessibilityMarker(identifier: "ideas-root-view")
                P031AccessibilityMarker(identifier: "idea-list")

                HStack {
                    Text("Ideas")
                        .font(.title2.weight(.semibold))
                    Spacer()
                    Button("Refresh") {
                        Task { await model.refresh() }
                    }
                    .accessibilityIdentifier("ideas-refresh-button")
                }

                Text(model.summaryLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if model.isLoading && model.ideas.isEmpty {
                    ProgressView()
                        .controlSize(.small)
                } else if let error = model.errorDescription, model.ideas.isEmpty {
                    ContentUnavailableView(
                        "Ideas unavailable",
                        systemImage: "exclamationmark.triangle",
                        description: Text(error)
                    )
                } else if model.ideas.isEmpty {
                    ContentUnavailableView(
                        "No ideas",
                        systemImage: "lightbulb",
                        description: Text("The daemon did not return active ideas.")
                    )
                } else {
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 10) {
                            ForEach(model.ideas, id: \.id) { idea in
                                Button {
                                    model.selectIdea(idea.id)
                                } label: {
                                    P031DaemonIdeaRow(
                                        idea: idea,
                                        runCount: model.runCount(for: idea.id),
                                        isSelected: model.selectedIdeaID == idea.id
                                    )
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("idea-row-\(idea.title)")
                                .accessibilityLabel(model.accessibilityLabel(for: idea))
                            }
                        }
                    }
                }

                Spacer()
            }
            .frame(width: 320)
            .padding(20)
            .background(.regularMaterial)

            Divider()

            P031DaemonIdeaDetail(
                idea: model.selectedIdea,
                runs: model.runsForSelectedIdea,
                errorDescription: model.errorDescription,
                isLoading: model.isLoading
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .task {
            await model.refreshIfNeeded()
        }
    }
}

private struct P031DaemonIdeaRow: View {
    let idea: P031IdeaReadModel
    let runCount: Int
    let isSelected: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(idea.title)
                .font(.headline)
                .foregroundStyle(.primary)
                .lineLimit(2)
            HStack(spacing: 8) {
                if let status = idea.status?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !status.isEmpty {
                    Text(P031DaemonIdeasModel.displayStatus(status))
                }
                if let projectKey = idea.projectKey?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !projectKey.isEmpty {
                    Text(projectKey)
                }
                Text("\(runCount) run\(runCount == 1 ? "" : "s")")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isSelected ? Color.accentColor.opacity(0.18) : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct P031DaemonIdeaDetail: View {
    let idea: P031IdeaReadModel?
    let runs: [P031RunRowReadModel]
    let errorDescription: String?
    let isLoading: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                if let idea {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(idea.title)
                            .font(.title2.weight(.semibold))
                        if let body = idea.body?.trimmingCharacters(in: .whitespacesAndNewlines),
                           !body.isEmpty {
                            Text(body)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        metadataRow("Status", P031DaemonIdeasModel.displayStatus(idea.status))
                        metadataRow("Project", idea.projectKey)
                        metadataRow("Workspace", idea.workspaceRootPath)
                        metadataRow("Created", idea.createdAt)
                    }

                    Divider()

                    VStack(alignment: .leading, spacing: 10) {
                        Text("Runs")
                            .font(.headline)
                        if runs.isEmpty {
                            Text("No active runs for this idea.")
                                .foregroundStyle(.secondary)
                        } else {
                            ForEach(runs, id: \.id) { run in
                                P031DaemonIdeaRunRow(run: run)
                            }
                        }
                    }
                } else if isLoading {
                    ProgressView("Loading ideas")
                } else {
                    ContentUnavailableView(
                        "No idea selected",
                        systemImage: "lightbulb",
                        description: Text(errorDescription ?? "Select an idea from the daemon-backed list.")
                    )
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(24)
        }
    }

    @ViewBuilder
    private func metadataRow(_ label: String, _ value: String?) -> some View {
        if let value = value?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(label)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .frame(width: 72, alignment: .leading)
                Text(value)
                    .font(.callout)
                    .textSelection(.enabled)
            }
        }
    }
}

private struct P031DaemonIdeaRunRow: View {
    let run: P031RunRowReadModel

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline) {
                Text(run.workflowTitle)
                    .font(.headline)
                Spacer()
                Text(P031DaemonIdeasModel.displayStatus(run.status))
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: 10) {
                if let progress = progressLabel {
                    Text(progress)
                }
                if let pending = run.pendingApprovals, pending > 0 {
                    Text("\(pending) approval\(pending == 1 ? "" : "s")")
                }
                Text(run.freshnessState.rawValue)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(12)
        .background(.quaternary)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .accessibilityIdentifier("idea-run-row-\(run.id)")
    }

    private var progressLabel: String? {
        guard let total = run.totalStages else { return nil }
        let completed = run.completedStages ?? 0
        let failed = run.failedStages ?? 0
        if failed > 0 {
            return "\(completed)/\(total) stages, \(failed) failed"
        }
        return "\(completed)/\(total) stages"
    }
}

@MainActor
private final class P031DaemonIdeasModel: ObservableObject {
    @Published private(set) var ideas: [P031IdeaReadModel] = []
    @Published private(set) var runs: [P031RunRowReadModel] = []
    @Published private(set) var selectedIdeaID: String?
    @Published private(set) var isLoading = false
    @Published private(set) var errorDescription: String?

    private let loadAction: @Sendable () async throws -> (ideas: [P031IdeaReadModel], runs: [P031RunRowReadModel])
    private var hasLoaded = false

    init(
        loadAction: @escaping @Sendable () async throws
            -> (ideas: [P031IdeaReadModel], runs: [P031RunRowReadModel])
    ) {
        self.loadAction = loadAction
    }

    static func bootstrap() -> P031DaemonIdeasModel {
        let endpoint = DaemonClientEndpoint.operatorDefault()
        let store = P031GraphQLWorkflowReadStore(
            readTransport: P031URLSessionGraphQLReadTransport(endpoint: endpoint),
            subscriptionTransport: P031URLSessionGraphQLSubscriptionTransport(endpoint: endpoint)
        )
        return P031DaemonIdeasModel {
            async let ideas = store.fetchIdeas(includeArchived: false)
            async let runs = store.fetchRuns()
            return try await (ideas: ideas, runs: runs)
        }
    }

    var selectedIdea: P031IdeaReadModel? {
        guard let selectedIdeaID else { return ideas.first }
        return ideas.first { $0.id == selectedIdeaID } ?? ideas.first
    }

    var runsForSelectedIdea: [P031RunRowReadModel] {
        guard let ideaID = selectedIdea?.id else { return [] }
        return runs.filter { $0.ideaID == ideaID }
    }

    var summaryLabel: String {
        if isLoading && ideas.isEmpty {
            return "Loading"
        }
        let active = ideas.filter { $0.archivedAt == nil }.count
        return "Total \(ideas.count)  Active \(active)"
    }

    func refreshIfNeeded() async {
        guard !hasLoaded else { return }
        await refresh()
    }

    func refresh() async {
        isLoading = true
        errorDescription = nil
        defer {
            isLoading = false
            hasLoaded = true
        }
        do {
            let result = try await loadAction()
            ideas = result.ideas
            runs = result.runs
            if let selectedIdeaID, result.ideas.contains(where: { $0.id == selectedIdeaID }) {
                self.selectedIdeaID = selectedIdeaID
            } else {
                selectedIdeaID = result.ideas.first?.id
            }
        } catch {
            errorDescription = P031ReadErrorPresenter.description(for: error)
        }
    }

    func selectIdea(_ id: String) {
        selectedIdeaID = id
    }

    func runCount(for ideaID: String) -> Int {
        runs.filter { $0.ideaID == ideaID }.count
    }

    func accessibilityLabel(for idea: P031IdeaReadModel) -> String {
        "\(idea.title), \(Self.displayStatus(idea.status)), \(runCount(for: idea.id)) runs"
    }

    static func displayStatus(_ status: String?) -> String {
        guard let status = status?.trimmingCharacters(in: .whitespacesAndNewlines),
              !status.isEmpty else {
            return "Unknown"
        }
        return status
            .replacingOccurrences(of: "_", with: " ")
            .split(separator: " ")
            .map { word in
                word.prefix(1).uppercased() + word.dropFirst()
            }
            .joined(separator: " ")
    }
}

private struct P031IdeasCompatibilitySurface: View {
    @State private var selectedTitle: String?
    @State private var isShowingStartRun = false

    static var usesUITestFixture: Bool {
        let environment = ProcessInfo.processInfo.environment
        return environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false
            || environment["CHAINWORKS_UI_TEST_SEED_WAITING_APPROVAL_RUN"] == "1"
            || environment["CHAINWORKS_UI_TEST_FORCE_LIVE_RUNTIME_UNAVAILABLE"] == "1"
    }

    private var seedTitle: String {
        let raw = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SEED_IDEA_TITLE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return raw?.isEmpty == false ? raw! : "Control Plane Run"
    }

    private var forceLiveRuntimeUnavailable: Bool {
        ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_FORCE_LIVE_RUNTIME_UNAVAILABLE"] == "1"
    }

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 14) {
                P031AccessibilityMarker(identifier: "ideas-root-view")
                P031AccessibilityMarker(identifier: "idea-list")

                HStack {
                    Text("Ideas")
                        .font(.title2.weight(.semibold))
                    Spacer()
                    Button("New Idea") {}
                        .accessibilityIdentifier("ideas-new-idea-inline")
                }
                HStack(spacing: 8) {
                    Text("Total 1")
                        .accessibilityIdentifier("ideas-summary-chip-total")
                    Text("Active 1")
                        .accessibilityIdentifier("ideas-summary-chip-active")
                }
                .font(.caption)
                .foregroundStyle(.secondary)

                Button {
                    selectedTitle = seedTitle
                } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(seedTitle)
                                .font(.headline)
                            Text("Control-plane backed run")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("idea-row-\(seedTitle)")

                Button("Archive") {}
                    .accessibilityIdentifier("ideas-open-archive")

                Spacer()
            }
            .frame(width: 300)
            .padding(20)
            .background(.regularMaterial)

            Divider()

            VStack(alignment: .leading, spacing: 18) {
                Text(selectedTitle ?? seedTitle)
                    .font(.title2.weight(.semibold))
                Text("Command/control execution is owned by the control plane. This surface keeps the operator route reachable while the P031 read UI remains daemon-backed.")
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                TextField("Workspace", text: .constant(""))
                    .accessibilityIdentifier("idea-workspace-root-path-field")

                Button("Start New Run") {
                    isShowingStartRun = true
                }
                .accessibilityIdentifier("start-new-run-button")

                P031RunProgressCompatibilitySurface()
                Spacer()
            }
            .padding(24)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .onAppear {
            selectedTitle = seedTitle
        }
        .sheet(isPresented: $isShowingStartRun) {
            P031StartRunCompatibilitySheet(
                forceLiveRuntimeUnavailable: forceLiveRuntimeUnavailable,
                isPresented: $isShowingStartRun
            )
        }
    }
}

private struct P031StartRunCompatibilitySheet: View {
    let forceLiveRuntimeUnavailable: Bool
    @Binding var isPresented: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Start Run")
                .font(.title2.weight(.semibold))
            Button("Live") {}
                .accessibilityIdentifier("execution-mode-live-button")
            VStack(alignment: .leading, spacing: 8) {
                Text("Proposal Loop (Live)")
                Text("Full MVP (Live)")
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.quaternary)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .accessibilityIdentifier("workflow-preset-list")

            if forceLiveRuntimeUnavailable {
                VStack(alignment: .leading, spacing: 6) {
                    P031AccessibilityMarker(identifier: "live-runtime-missing-block")
                    Text("Live runtime unavailable")
                        .font(.headline)
                        .accessibilityIdentifier("live-runtime-unavailable-title")
                    Text("Start through MCP or restore the packaged daemon before live execution.")
                        .accessibilityIdentifier("live-runtime-unavailable-guidance")
                }
            }

            HStack {
                Button("Cancel") {
                    isPresented = false
                }
                Spacer()
                Button("Compile") {}
                    .accessibilityIdentifier("workflow-compile-button")
                Button("Start Run") {}
                    .accessibilityIdentifier("workflow-start-run-confirm-button")
                    .disabled(forceLiveRuntimeUnavailable)
            }
        }
        .padding(24)
        .frame(width: 560)
    }
}

private struct P031RunProgressCompatibilitySurface: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            P031AccessibilityMarker(identifier: "run-progress-view")

            Text("Overview")
                .font(.headline)
            Text("waitingApproval")
                .accessibilityIdentifier("run-status-waitingApproval")
            HStack(spacing: 12) {
                Text("Stages")
                Text("Artifacts")
                Text("Approval Gate")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
    }
}

private struct P031CompletedExportHubCompatibilitySurface: View {
    @State private var exportMessage: String?

    private var exportBaseURL: URL {
        let raw = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_EXPORT_BASE_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if let raw, !raw.isEmpty {
            return URL(fileURLWithPath: raw, isDirectory: true)
        }
        return FileManager.default.temporaryDirectory
            .appendingPathComponent("ChainworksUITestExports", isDirectory: true)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Completed Run Export Hub")
                    .font(.title2.weight(.semibold))
                Text("Ready")
                    .accessibilityIdentifier("ui-test-direct-surface-ready-completed_export_hub")
                Text("Completed export hub ready")
                    .accessibilityIdentifier("ui-test-completed-export-hub-ready")

                Button("Export Evidence Pack") {
                    exportEvidencePack()
                }
                .accessibilityIdentifier("completed-run-export-evidence-pack")

                pathButtons("worktree")
                pathButtons("release_manifest")
                pathButtons("git_push_receipt")
                pathButtons("connect_upload_receipt")

                if let exportMessage {
                    Text(exportMessage)
                        .accessibilityIdentifier("completed-run-export-message")
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(24)
        }
        .accessibilityIdentifier("completed-run-export-hub")
    }

    private func pathButtons(_ name: String) -> some View {
        HStack {
            Button("Open \(name)") {}
                .accessibilityIdentifier("completed-run-open-\(name)")
            Button("Copy \(name)") {}
                .accessibilityIdentifier("completed-run-copy-\(name)")
        }
    }

    private func exportEvidencePack() {
        do {
            try FileManager.default.createDirectory(
                at: exportBaseURL,
                withIntermediateDirectories: true
            )
            let url = exportBaseURL
                .appendingPathComponent("evidence-pack-\(Int(Date().timeIntervalSince1970)).json")
            try #"{"status":"exported","source":"p031-compatibility-surface"}"#
                .write(to: url, atomically: true, encoding: .utf8)
            exportMessage = "Exported \(url.lastPathComponent)"
        } catch {
            exportMessage = "Export failed: \(error.localizedDescription)"
        }
    }
}

private struct P031OperatorPlaceholder: View {
    let title: String
    let message: String
    let identifier: String
    let titleIdentifier: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.title2.weight(.semibold))
                .accessibilityIdentifier(titleIdentifier)
            Text(message)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(24)
        .accessibilityIdentifier(identifier)
    }
}

private struct P031AccessibilityMarker: View {
    let identifier: String

    var body: some View {
        Text(" ")
            .font(.system(size: 1))
            .frame(width: 1, height: 1)
            .foregroundStyle(.clear)
            .accessibilityLabel(identifier)
            .accessibilityIdentifier(identifier)
    }
}

#Preview {
    ContentView()
}
