import Combine
import SwiftUI

extension Notification.Name {
    static let chainworksSelectTab = Notification.Name("chainworks.selectTab")
    static let chainworksOpenRunInRunsHome = Notification.Name("chainworks.openRunInRunsHome")
    // P036: deep-link from blocked/failed Runs to Settings System Readiness section
    static let chainworksOpenSystemReadiness = Notification.Name("chainworks.openSystemReadiness")
    // P036: signal RunsHomeView to scroll/select into the waiting-approval lane
    static let chainworksFocusWaitingApprovalLane = Notification.Name("chainworks.focusWaitingApprovalLane")
}

struct ContentView: View {
    @State private var selectedTab: Tab
    @State private var definitionsSegmentRequest: DefinitionsView.Segment? = nil
    // P036: return target so operators can navigate back to the originating run after
    // inspecting System Readiness from a blocked/failed run detail.
    @State private var systemReadinessReturnRunID: String? = nil
    @StateObject private var daemonStatus = DaemonStatusViewModel.bootstrap()
    @StateObject private var schedulerHealth = SchedulerHealthViewModel.bootstrap()
    @StateObject private var runsModel = P031ThinReadDashboardModel.bootstrap()
    @StateObject private var workbench = RunsWorkbenchPresentationModel()

    private let forcedInitialTab: Tab?
    private let forcedUISurface: UISurface?
    // P036: set true when CHAINWORKS_UI_TEST_INITIAL_TAB maps to "approvals" so
    // RunsHomeView can focus the waiting-approval lane after the view hierarchy loads.
    private let focusWaitingApprovalOnLoad: Bool

    // P036 cutover: legacy Approvals routes are mapped into Runs with waiting-approval focus;
    // there is no standalone top-level Approvals tab.
    enum Tab: String {
        case runs = "Runs"
        case ideas = "Ideas"
        case definitions = "Definitions"
        case settings = "Settings"

        static func from(rawValue: String) -> Tab? {
            switch rawValue {
            case "Runs Home", "runsHome", "Runs": return .runs
            case "Ideas", "ideas": return .ideas
            // P036 old_route_mapping: legacy Approvals routes redirect to Runs.
            case "Approvals", "approvals": return .runs
            case "Agent Catalog", "agentCatalog", "Definitions": return .definitions
            case "Workflow Inspector", "workflowInspector": return .definitions
            case "Pilot Readiness", "pilotReadiness", "Settings", "providerSettings": return .settings
            default: return Tab(rawValue: rawValue)
            }
        }
    }

    enum UISurface: String {
        case completedExportHub = "completed_export_hub"
        case p077CloseoutReadiness = "p077_closeout_readiness"
    }


    init() {
        let environment = ProcessInfo.processInfo.environment
        let rawInitialTab = environment["CHAINWORKS_UI_TEST_INITIAL_TAB"] ?? ""
        let initialTab = Tab.from(rawValue: rawInitialTab)
        forcedInitialTab = initialTab
        focusWaitingApprovalOnLoad = rawInitialTab == "Approvals" || rawInitialTab == "approvals"
        #if DEBUG
        forcedUISurface = environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"]
            .flatMap(UISurface.init(rawValue:))
        #else
        forcedUISurface = nil
        #endif
        _selectedTab = State(initialValue: initialTab ?? .runs)
        // P036 initial segment routing: map legacy route names to Definitions segments
        // so CHAINWORKS_UI_TEST_INITIAL_TAB=workflowInspector opens the Workflow segment
        // rather than the default Agents segment.
        switch rawInitialTab {
        case "workflowInspector", "Workflow Inspector":
            _definitionsSegmentRequest = State(initialValue: .workflows)
        case "agentCatalog", "Agent Catalog":
            _definitionsSegmentRequest = State(initialValue: .agents)
        default:
            break
        }
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
                            onOpenSchedulerHealth: { selectedTab = .settings }
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
                .task {
                    await runsModel.loadIfNeeded()
                }
            }
        }
    }

    private var tabShell: some View {
        TabView(selection: $selectedTab) {
            tabContent(.runs) {
                RunsHomeView(model: runsModel, workbench: workbench, initialTab: .overview)
            }
            .tabItem { Label("Runs", systemImage: "house") }
            .tag(Tab.runs)
            .badge(runsModel.totalPendingApprovalCount > 0 ? runsModel.totalPendingApprovalCount : 0)

            tabContent(.ideas) {
                if P031IdeasCompatibilitySurface.usesUITestFixture {
                    P031IdeasCompatibilitySurface()
                } else {
                    P031DaemonIdeasSurface()
                }
            }
            .tabItem { Label("Ideas", systemImage: "lightbulb") }
            .tag(Tab.ideas)

            tabContent(.definitions) {
                DefinitionsView(
                    catalogURL: exampleFileURL(
                        environmentKey: "CHAINWORKS_AGENT_CATALOG_SOURCE_PATH",
                        bundleName: "agents",
                        repoRelativePath: "examples/agents/agents.yaml"
                    ),
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
                    segmentRequest: $definitionsSegmentRequest
                )
            }
            .tabItem { Label("Definitions", systemImage: "square.grid.2x2") }
            .tag(Tab.definitions)

            tabContent(.settings) {
                SettingsView(
                    runsModel: runsModel,
                    workbench: workbench,
                    returnRunID: systemReadinessReturnRunID,
                    onClearReturnRunID: { systemReadinessReturnRunID = nil }
                )
            }
            .tabItem { Label("Settings", systemImage: "slider.horizontal.3") }
            .tag(Tab.settings)

        }
        .task(id: forcedInitialTab?.rawValue ?? "default") {
            guard let forcedInitialTab, selectedTab != forcedInitialTab else { return }
            selectedTab = forcedInitialTab
        }
        // P036: when CHAINWORKS_UI_TEST_INITIAL_TAB=approvals, post the waiting-approval
        // focus notification after the view has loaded and data may have settled.
        .task(id: focusWaitingApprovalOnLoad ? "approvals-focus" : "noop") {
            guard focusWaitingApprovalOnLoad else { return }
            // PC-003: set workbench flag so RunsHomeView handles it on mount even if
            // lanes haven't loaded yet (onChange initial:true picks it up).
            workbench.requestFocusWaitingApprovalLane()
            NotificationCenter.default.post(name: .chainworksFocusWaitingApprovalLane, object: nil)
        }
        .onReceive(NotificationCenter.default.publisher(for: .chainworksSelectTab)) { notification in
            let rawValueFromUserInfo = notification.userInfo?["tab"] as? String
            let rawValueFromObject = notification.object as? String
            let rawValue = rawValueFromUserInfo ?? rawValueFromObject

            guard let rawValue, let tab = Tab.from(rawValue: rawValue) else {
                #if DEBUG
                if let rawValue {
                    print("[P036] chainworksSelectTab: unknown rawValue '\(rawValue)'")
                }
                #endif
                return
            }
            let previousTab = selectedTab
            selectedTab = tab
            P036UICounters.shared.recordTabRouteResolution(
                source: previousTab.rawValue,
                target: tab.rawValue,
                result: "routed"
            )

            // Segment routing: honor both camelCase and title-case alias forms so
            // CHAINWORKS_UI_TEST_INITIAL_TAB and notification routing stay in sync.
            switch rawValue {
            case "workflowInspector", "Workflow Inspector":
                definitionsSegmentRequest = .workflows
            case "agentCatalog", "Agent Catalog":
                definitionsSegmentRequest = .agents
            case "Approvals", "approvals":
                // PC-003: set workbench flag before posting notification. The flag survives
                // the tab-switch render cycle so RunsHomeView picks it up on mount even
                // if the notification fires before the view is in the hierarchy.
                workbench.requestFocusWaitingApprovalLane()
                NotificationCenter.default.post(name: .chainworksFocusWaitingApprovalLane, object: nil)
            default:
                break
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenRunInRunsHome)) { _ in
            selectedTab = .runs
        }
        .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenSystemReadiness)) { _ in
            // Capture the currently selected run as a return target before switching tabs,
            // so the operator can navigate back to the exact run after inspecting readiness.
            systemReadinessReturnRunID = runsModel.selectedRunID
            selectedTab = .settings
        }
        .onChange(of: runsModel.runsHome) {
            if let newValue = runsModel.runsHome {
                workbench.populate(from: newValue)
            }
        }
        .onChange(of: runsModel.runDetail) {
            if let newValue = runsModel.runDetail {
                workbench.populate(from: newValue)
            }
        }
        .onChange(of: runsModel.daemonLifecycle) {
            workbench.populate(daemon: runsModel.daemonLifecycle, scheduler: runsModel.schedulerHealth)
        }
        .onChange(of: runsModel.schedulerHealth) {
            workbench.populate(daemon: runsModel.daemonLifecycle, scheduler: runsModel.schedulerHealth)
        }
        .onOpenURL { url in
            guard url.scheme == "chainworks" else { return }
            switch url.host {
            case "runs":
                selectedTab = .runs
                if let runID = url.pathComponents.last, runID != "/" {
                    runsModel.selectRun(runID)
                }
            case "ideas":
                selectedTab = .ideas
            case "definitions":
                selectedTab = .definitions
            case "settings":
                selectedTab = .settings
            case "approvals":
                selectedTab = .runs
                // PC-003: set workbench flag before notification for same race fix as selectTab path.
                workbench.requestFocusWaitingApprovalLane()
                NotificationCenter.default.post(name: .chainworksFocusWaitingApprovalLane, object: nil)
            default:
                break
            }
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
#if DEBUG
            P031CompletedExportHubCompatibilitySurface()
#else
            RunsHomeView(workbench: workbench)
#endif
        case .p077CloseoutReadiness:
#if DEBUG
            ZStack(alignment: .topLeading) {
                RunsHomeView(
                    model: P031ThinReadDashboardModel.previewLoadedWithCloseoutReadiness(),
                    workbench: workbench,
                    initialTab: .overview
                )
                P031AccessibilityMarker(identifier: "ui-test-direct-surface-ready-p077_closeout_readiness")
                    .frame(width: 1, height: 1)
                    .opacity(0.01)
            }
#else
            RunsHomeView(workbench: workbench)
#endif
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
                        metadataRow("Workspace", idea.workspaceRootPath.map(redactedPath))
                        metadataRow("Created", idea.createdAt)
                    }

                    Divider()

            VStack(alignment: .leading, spacing: 10) {
                Text("Run Status")
                    .font(.headline)
                
                // P036: use the canonical projected lane. .deferred surfaces unknown server
                // statuses as an explicit projection-lag row rather than silently miscounting.
                let waitingCount = runs.filter { $0.lane == .waiting }.count
                let blockedCount = runs.filter { $0.lane == .blocked }.count
                let runningCount = runs.filter { $0.lane == .running }.count
                let completedCount = runs.filter { $0.lane == .completed }.count
                let deferredCount = runs.filter { $0.lane == .deferred }.count

                VStack(alignment: .leading, spacing: 8) {
                    compactStatusStrip(label: "Waiting Approval", count: waitingCount, color: .orange)
                    compactStatusStrip(label: "Blocked or Failed", count: blockedCount, color: .red)
                    compactStatusStrip(label: "Running", count: runningCount, color: .blue)
                    compactStatusStrip(label: "Completed", count: completedCount, color: .green)
                    if deferredCount > 0 {
                        compactStatusStrip(label: "Status Unknown", count: deferredCount, color: .gray)
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

    private func redactedPath(_ path: String) -> String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        if path.hasPrefix(home) { return "~" + path.dropFirst(home.count) }
        if path.hasPrefix("/") { return "<redacted>" }
        return path
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

    private func compactStatusStrip(label: String, count: Int, color: Color) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
            Spacer()
            Text("\(count)")
                .font(.caption.monospacedDigit().bold())
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(color.opacity(0.15))
                .foregroundStyle(color)
                .clipShape(Capsule())
            
            if count > 0 {
                Button {
                    NotificationCenter.default.post(
                        name: .chainworksSelectTab,
                        object: nil,
                        userInfo: ["tab": "Runs"]
                    )
                } label: {
                    Image(systemName: "chevron.right")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(Color.primary.opacity(0.04))
        .cornerRadius(8)
    }
}

private struct P031DaemonIdeaRunRow: View {
    let run: P031RunRowReadModel

    var body: some View {
        HStack(spacing: 8) {
            Text(run.workflowTitle)
                .font(.subheadline.weight(.medium))
                .lineLimit(1)
            Spacer()
            if let pending = run.pendingApprovals, pending > 0 {
                Image(systemName: "checkmark.seal.fill")
                    .foregroundStyle(.orange)
                    .font(.caption)
                Text("\(pending)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
            Text(P031DaemonIdeasModel.displayStatus(run.status))
                .font(.caption2.weight(.bold))
                .padding(.horizontal, 4)
                .padding(.vertical, 1)
                .background(.quaternary)
                .clipShape(Capsule())
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.quaternary.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .accessibilityIdentifier("idea-run-row-\(run.id)")
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

                P031RunProgressCompatibilitySurface()
                Spacer()
            }
            .padding(24)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .onAppear {
            selectedTitle = seedTitle
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
                    .disabled(true)
                Button("Start Run") {}
                    .accessibilityIdentifier("workflow-start-run-confirm-button")
                    .disabled(true)
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
        #if DEBUG
        let raw = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_EXPORT_BASE_PATH"]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if let raw, !raw.isEmpty {
            return URL(fileURLWithPath: raw, isDirectory: true)
        }
        #endif
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


// P036 Phase 2c: production allCases returns the four consolidated tabs only.
// Legacy Approvals routes are compatibility aliases, not tab cases.
extension ContentView.Tab: CaseIterable {
    static var allCases: [ContentView.Tab] {
        [.runs, .ideas, .definitions, .settings]
    }
}

#Preview {
    ContentView()
}
