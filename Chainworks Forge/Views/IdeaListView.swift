import SwiftUI
import SwiftData

struct IdeaListView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Query(sort: \Idea.createdAt, order: .reverse) private var ideas: [Idea]
    @State private var newTitle = ""
    @State private var newBody = ""
    @State private var newAttachmentPath = ""
    @State private var showNewIdeaSheet = false
    @State private var showArchivedIdeas = false

    private var activeIdeas: [Idea] {
        ideas.filter { !$0.isArchived }
    }

    private var archivedIdeas: [Idea] {
        ideas.filter(\.isArchived)
    }

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                // Summary strip (UI-001)
                summaryStrip

                Group {
                    if activeIdeas.isEmpty {
                        ContentUnavailableView(
                            archivedIdeas.isEmpty ? "No ideas yet" : "No active ideas",
                            systemImage: "lightbulb",
                            description: Text(archivedIdeas.isEmpty ? "Create your first idea to get started." : "Open the archive lane to restore an idea or create a new one.")
                        )
                    } else {
                        List {
                            ForEach(activeIdeas) { idea in
                                NavigationLink {
                                    IdeaDetailView(idea: idea)
                                } label: {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(idea.title).font(.headline)
                                        HStack(spacing: 8) {
                                            IdeaLifecycleBadge(idea: idea)
                                            if idea.isArchived {
                                                Image(systemName: "archivebox.fill")
                                                    .font(.caption2)
                                                    .foregroundStyle(.secondary)
                                            }
                                            if idea.attachmentPath != nil {
                                                Image(systemName: "paperclip")
                                                    .font(.caption2)
                                                    .foregroundStyle(.secondary)
                                            }
                                            // Show active run indicator
                                            if idea.runs.contains(where: { [.running, .waitingApproval, .pending, .ready].contains($0.status) }) {
                                                Image(systemName: "play.circle.fill")
                                                    .font(.caption2)
                                                    .foregroundStyle(.green)
                                            }
                                        }
                                    }
                                }
                                .contextMenu {
                                    ideaArchiveMenu(for: idea)
                                }
                                .accessibilityIdentifier("idea-row-\(idea.title)")
                            }
                            .onDelete(perform: deleteIdeas)
                        }
                        .accessibilityIdentifier("idea-list")
                    }
                }

                // Pending approvals bar
                if executionService.pendingApprovalCount > 0 {
                    approvalBar
                }
            }
            .navigationSplitViewColumnWidth(min: 200, ideal: 250)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { showArchivedIdeas = true }) {
                        Label("Archive", systemImage: "archivebox")
                    }
                    .accessibilityIdentifier("ideas-open-archive")
                }
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { showNewIdeaSheet = true }) {
                        Label("New Idea", systemImage: "plus")
                    }
                }
            }
            .sheet(isPresented: $showNewIdeaSheet) {
                newIdeaSheet
            }
            .sheet(isPresented: $showArchivedIdeas) {
                ArchivedIdeasView()
                    .environment(\.modelContext, modelContext)
            }
            .accessibilityIdentifier("ideas-root-view")
        } detail: {
            Text("Select an idea")
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Summary Strip

    private var summaryStrip: some View {
        let draftCount = activeIdeas.filter { $0.status == .draft }.count
        let activeCount = activeIdeas.filter { $0.status == .active }.count

        return HStack {
            Image(systemName: "lightbulb.fill")
                .foregroundStyle(.blue)
            Text("\(activeIdeas.count) ideas \u{00B7} \(draftCount) drafts \u{00B7} \(activeCount) active")
            if !archivedIdeas.isEmpty {
                Button {
                    showArchivedIdeas = true
                } label: {
                    Label("\(archivedIdeas.count) archived", systemImage: "archivebox")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("ideas-summary-open-archive")
            }
            Spacer()
            if executionService.hasActiveRuns {
                Label("\(executionService.activeOrchestrators.count) running", systemImage: "bolt.fill")
                    .font(.caption)
                    .foregroundStyle(.green)
            }
            switch executionService.liveRuntimeReadiness {
            case .ready(_, let source):
                Label("Live ready (\(source))", systemImage: "bolt.horizontal.circle.fill")
                    .font(.caption)
                    .foregroundStyle(.green)
                    .accessibilityIdentifier("live-runtime-ready")
            case .unavailable:
                Label("Live unavailable", systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.orange)
                    .accessibilityIdentifier("live-runtime-unavailable")
            }
        }
        .font(.caption)
        .padding(.horizontal)
        .padding(.vertical, 6)
        .background(Color.blue.opacity(0.08))
    }

    // MARK: - Approval Bar

    private var approvalBar: some View {
        HStack {
            Image(systemName: "checkmark.seal.fill")
                .foregroundStyle(.orange)
            Text("\(executionService.pendingApprovalCount) pending approval(s)")
            Spacer()
        }
        .font(.caption)
        .padding(.horizontal)
        .padding(.vertical, 6)
        .background(Color.orange.opacity(0.1))
    }

    // MARK: - New Idea Sheet

    private var newIdeaSheet: some View {
        VStack(spacing: 16) {
            Text("New Idea").font(.headline)
            TextField("Title", text: $newTitle)
                .textFieldStyle(.roundedBorder)
            TextEditor(text: $newBody)
                .frame(minHeight: 100)
                .border(Color.secondary.opacity(0.3))
            HStack {
                TextField("Attachment path (optional)", text: $newAttachmentPath)
                    .textFieldStyle(.roundedBorder)
                Button("Browse...") { browseAttachment() }
            }
            HStack {
                Button("Cancel") {
                    resetForm()
                    showNewIdeaSheet = false
                }
                Spacer()
                Button("Save Idea") {
                    createIdea()
                    showNewIdeaSheet = false
                }
                .disabled(newTitle.trimmingCharacters(in: .whitespaces).isEmpty)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding()
        .frame(minWidth: 400, minHeight: 300)
    }

    private func createIdea() {
        let trimmedPath = newAttachmentPath.trimmingCharacters(in: .whitespaces)
        let idea = Idea(
            title: newTitle.trimmingCharacters(in: .whitespaces),
            body: newBody.trimmingCharacters(in: .whitespaces),
            attachmentPath: trimmedPath.isEmpty ? nil : trimmedPath
        )
        modelContext.insert(idea)
        try? modelContext.save()
        resetForm()
    }

    private func resetForm() {
        newTitle = ""
        newBody = ""
        newAttachmentPath = ""
    }

    private func deleteIdeas(offsets: IndexSet) {
        for index in offsets {
            modelContext.delete(activeIdeas[index])
        }
        try? modelContext.save()
    }

    private func statusLabel(for idea: Idea) -> String {
        if idea.isArchived {
            return "Archived"
        }
        return idea.status.rawValue.capitalized
    }

    @MainActor
    @ViewBuilder
    private func ideaArchiveMenu(for idea: Idea) -> some View {
        let service = IdeaArchiveService(modelContext: modelContext)

        if idea.isArchived {
            Button("Restore") {
                try? service.restore(idea)
            }
        } else {
            let eligibility = IdeaArchivePolicy.eligibility(for: idea)
            Button("Archive") {
                try? service.archive(idea)
            }
            .disabled(!eligibility.canArchive)
            if let reason = eligibility.reason {
                Text(reason)
            }
        }
    }

    private func browseAttachment() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            newAttachmentPath = url.path
        }
        #endif
    }
}

// MARK: - IdeaDetailView (enhanced for Proposal 002 + 004 — Start Run + Run Navigation)

struct IdeaDetailView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    let idea: Idea
    @State private var showStartRunSheet = false
    @State private var activeRun: Run?
    @State private var archiveMessage: String?

    /// Whether this idea has an active run (prevents starting another).
    private var hasActiveRun: Bool {
        idea.runs.contains { [.pending, .ready, .running, .waitingApproval, .blocked].contains($0.status) }
    }

    private var latestActiveRun: Run? {
        idea.runs
            .filter { [.pending, .ready, .running, .waitingApproval, .blocked].contains($0.status) }
            .sorted { $0.startedAt > $1.startedAt }
            .first
    }

    var body: some View {
        Group {
            if let activeRun {
                WorkflowRunProgressView(run: activeRun)
            } else {
                Form {
                    Section("Idea") {
                        LabeledContent("Title", value: idea.title)
                        LabeledContent("Status", value: idea.status.rawValue.capitalized)
                        LabeledContent("Created", value: idea.createdAt, format: .dateTime)
                        if let archivedAt = idea.archivedAt {
                            LabeledContent("Archived", value: archivedAt, format: .dateTime)
                        }
                        if let path = idea.attachmentPath {
                            LabeledContent("Attachment", value: path)
                        }
                    }

                    Section("Body") {
                        Text(idea.body)
                            .textSelection(.enabled)
                    }

                    Section("Archive") {
                        if idea.isArchived {
                            Label("Archived ideas stay visible here and in the archive lane until restored.", systemImage: "archivebox.fill")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Button {
                                restoreIdea()
                            } label: {
                                Label("Restore Idea", systemImage: "arrow.uturn.backward")
                            }
                            .buttonStyle(.borderedProminent)
                            .accessibilityIdentifier("restore-idea-button")
                        } else {
                            let eligibility = IdeaArchivePolicy.eligibility(for: idea)
                            Button {
                                archiveIdea()
                            } label: {
                                Label("Archive Idea", systemImage: "archivebox")
                            }
                            .buttonStyle(.bordered)
                            .disabled(!eligibility.canArchive)
                            .accessibilityIdentifier("archive-idea-button")

                            if let reason = eligibility.reason {
                                Text(reason)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .accessibilityIdentifier("archive-idea-reason")
                            } else {
                                Text("Archive the idea when it is draft or its latest run is terminal.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        if let archiveMessage {
                            Text(archiveMessage)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("archive-idea-message")
                        }
                    }

                    // Proposal 002 + 004: Start New Run action
                    Section {
                        Button {
                            showStartRunSheet = true
                        } label: {
                            Label("Start New Run", systemImage: "play.fill")
                        }
                        .disabled(hasActiveRun || idea.isArchived)
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("start-new-run-button")

                        if idea.isArchived {
                            Text("Restore the idea before starting a new run.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        } else if hasActiveRun {
                            Text("An active run already exists for this idea.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    Section("Runs") {
                        if idea.runs.isEmpty {
                            Text("No runs yet")
                                .foregroundStyle(.secondary)
                        } else {
                            ForEach(idea.runs.sorted(by: { $0.startedAt > $1.startedAt })) { run in
                                Button {
                                    activeRun = run
                                } label: {
                                    HStack {
                                        runStatusIcon(run.status)
                                        VStack(alignment: .leading, spacing: 2) {
                                            Text(run.workflowTitle)
                                                .font(.headline)
                                            HStack(spacing: 8) {
                                                Text(run.status.rawValue)
                                                    .font(.caption)
                                                    .foregroundStyle(statusColor(run.status))
                                                Text(run.startedAt, format: .dateTime)
                                                    .font(.caption2)
                                                    .foregroundStyle(.tertiary)
                                                if let cost = run.totalCostCents {
                                                    Text("\(cost)\u{00A2}")
                                                        .font(.caption2)
                                                        .foregroundStyle(.tertiary)
                                                }
                                            }
                                        }
                                        Spacer()
                                        if run.status == .waitingApproval {
                                            Image(systemName: "checkmark.seal.fill")
                                                .foregroundStyle(.orange)
                                        }
                                    }
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("run-row-\(run.workflowTitle)")
                            }
                        }
                    }
                }
            }
        }
        .task(id: idea.id) {
            if activeRun == nil {
                activeRun = latestActiveRun
            }
        }
        .formStyle(.grouped)
        .navigationTitle(idea.title)
        .sheet(isPresented: $showStartRunSheet) {
            WorkflowStartRunSheet(idea: idea) { prepared in
                showStartRunSheet = false
                activeRun = prepared.run
                Task { @MainActor in
                    // Let the sheet dismiss and detail view render before mutating execution state.
                    await Task.yield()
                    idea.status = .active
                    try? modelContext.save()
                    executionService.startRun(
                        run: prepared.run,
                        plan: prepared.plan,
                        workspace: prepared.workspace
                    )
                }
            }
        }
    }

    @MainActor
    private func archiveIdea() {
        let service = IdeaArchiveService(modelContext: modelContext)
        do {
            try service.archive(idea)
            archiveMessage = "Archived idea."
        } catch {
            archiveMessage = error.localizedDescription
        }
    }

    @MainActor
    private func restoreIdea() {
        let service = IdeaArchiveService(modelContext: modelContext)
        do {
            try service.restore(idea)
            archiveMessage = "Restored idea."
        } catch {
            archiveMessage = error.localizedDescription
        }
    }

    private func runStatusIcon(_ status: RunStatus) -> some View {
        let (icon, color): (String, Color) = {
            switch status {
            case .pending, .ready: return ("clock", .gray)
            case .running: return ("play.circle.fill", .green)
            case .waitingApproval: return ("checkmark.seal", .orange)
            case .blocked: return ("pause.circle.fill", .yellow)
            case .completed: return ("checkmark.circle.fill", .green)
            case .failed: return ("xmark.circle.fill", .red)
            case .cancelled: return ("stop.circle.fill", .gray)
            }
        }()
        return Image(systemName: icon).foregroundStyle(color)
    }

    private func statusColor(_ status: RunStatus) -> Color {
        switch status {
        case .pending, .ready: return .gray
        case .running: return .green
        case .waitingApproval: return .orange
        case .blocked: return .yellow
        case .completed: return .green
        case .failed: return .red
        case .cancelled: return .gray
        }
    }
}

// MARK: - Legacy Inline Views (replaced by standalone view files in Proposal 004)
// WorkflowStartRunSheet -> StartRunSheet.swift
// WorkflowRunProgressView -> RunProgressView.swift
// WorkflowStageDetailView -> StageDetailView.swift
// WorkflowArtifactInspectorView -> ArtifactInspectorView.swift

#if true
struct PreparedRunStart {
    let run: Run
    let plan: RunPlan
    let workspace: RunWorkspace
}

struct WorkflowStartRunSheet: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry

    let idea: Idea
    var onRunPrepared: ((PreparedRunStart) -> Void)? = nil

    @State private var compileState: CompileState = .idle
    @State private var compiledPlan: RunPlan?
    @State private var workflowURLs: [WorkflowPreset: URL] = [:]
    @State private var catalogURL: URL?
    @State private var isStarting = false
    @State private var selectedMode: ExecutionMode = .simulated
    @State private var selectedWorkflow: WorkflowPreset = .canonicalRelease
    @State private var startOptions: RunStartOptions = .empty
    @State private var preflightReport: PreflightReport?
    @State private var showPreflightSheet = false
    @State private var allowWarnStart = false
    @State private var showAdvancedOverrides = false

    private enum CompileState: Equatable {
        case idle
        case compiling
        case success(stateCount: Int, agentCount: Int)
        case error(String)
    }

    private enum ExecutionMode: String, CaseIterable, Identifiable {
        case simulated
        case live

        var id: String { rawValue }

        var title: String {
            switch self {
            case .simulated: return "Simulated"
            case .live: return "Live"
            }
        }
    }

    private enum WorkflowPreset: String, CaseIterable, Identifiable {
        case canonicalRelease
        case proposalLoopLive
        case fullMVPLive

        var id: String { rawValue }

        var mode: ExecutionMode {
            switch self {
            case .canonicalRelease: return .simulated
            case .proposalLoopLive: return .live
            case .fullMVPLive: return .live
            }
        }

        var title: String {
            switch self {
            case .canonicalRelease: return "Canonical Workflow"
            case .proposalLoopLive: return "Proposal Loop (Live)"
            case .fullMVPLive: return "Full MVP (Live)"
            }
        }

        var relativePath: String {
            switch self {
            case .canonicalRelease:
                return "examples/workflows/workflow.yaml"
            case .proposalLoopLive:
                return "examples/workflows/proposal-loop-live.yaml"
            case .fullMVPLive:
                return "examples/workflows/full-mvp-live.yaml"
            }
        }

        var bundleResourceName: String? {
            switch self {
            case .canonicalRelease:
                return "workflow"
            case .proposalLoopLive:
                return "proposal-loop-live"
            case .fullMVPLive:
                return "full-mvp-live"
            }
        }
    }

    private var availableWorkflows: [WorkflowPreset] {
        WorkflowPreset.allCases.filter { $0.mode == selectedMode }
    }

    private var selectedWorkflowURL: URL? {
        workflowURLs[selectedWorkflow]
    }

    private var workflowSelection: Binding<WorkflowPreset> {
        Binding(
            get: {
                if availableWorkflows.contains(selectedWorkflow) {
                    return selectedWorkflow
                }
                return availableWorkflows.first ?? .canonicalRelease
            },
            set: { newValue in
                selectedWorkflow = newValue
            }
        )
    }

    private var availableModes: [ExecutionMode] {
        executionService.supportsLiveExecution ? ExecutionMode.allCases : [.simulated]
    }

    private var liveModeConfigured: Bool {
        executionService.supportsLiveExecution
    }

    private var liveModeRequiresConfiguration: Bool {
        selectedMode == .live && !liveModeConfigured
    }

    private var preflightBlocksStart: Bool {
        preflightReport?.status == .fail
    }

    private var requiresCleanPreflight: Bool {
        providerSettingsStore.settings.runStartRequiresCleanPreflight
    }

    private var warnRequiresConfirmation: Bool {
        preflightReport?.status == .warn && !requiresCleanPreflight
    }

    var body: some View {
        launchConfigurationBody
    }

    private var launchConfigurationBody: some View {
        VStack(spacing: 16) {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 12) {
                Image(systemName: "play.circle.fill")
                    .font(.title2)
                    .foregroundStyle(.blue)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Start New Run")
                        .font(.headline)
                    Text("Compile YAML into an immutable RunPlan snapshot, then create the run.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }

            GroupBox("Idea") {
                VStack(alignment: .leading, spacing: 6) {
                    Text(idea.title)
                        .font(.subheadline.bold())
                    Text(idea.body)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(4)
                    if let attachmentPath = idea.attachmentPath, !attachmentPath.isEmpty {
                        Label(attachmentPath, systemImage: "paperclip")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Run Mode") {
                VStack(alignment: .leading, spacing: 10) {
                    Picker("Execution Mode", selection: $selectedMode) {
                        ForEach(availableModes) { mode in
                            Text(mode.title)
                                .accessibilityIdentifier("execution-mode-\(mode.id)")
                                .tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("execution-mode-picker")

                    Picker("Workflow", selection: workflowSelection) {
                        ForEach(availableWorkflows) { workflow in
                            Text(workflow.title)
                                .accessibilityIdentifier("workflow-preset-\(workflow.id)")
                                .tag(workflow)
                        }
                    }
                    .accessibilityIdentifier("workflow-preset-picker")

                    if !liveModeConfigured {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack(spacing: 6) {
                                Image(systemName: "exclamationmark.triangle.fill")
                                Text("Live runtime is unavailable.")
                                    .accessibilityIdentifier("live-runtime-unavailable-title")
                            }
                            .accessibilityElement(children: .contain)
                            .accessibilityIdentifier("live-runtime-unavailable-header")
                            Text("Connect a Goose backend or enable the fixture backend to unlock `Proposal Loop (Live)`.")
                                .accessibilityIdentifier("live-runtime-unavailable-guidance")
                            Text("Advanced setup: `CHAINWORKS_GOOSE_BASE_URL` or `CHAINWORKS_GOOSE_FIXTURE_MODE=proposal_loop_success`, then relaunch the app.")
                                .accessibilityIdentifier("live-runtime-unavailable-advanced")
                        }
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .accessibilityElement(children: .contain)
                        .accessibilityIdentifier("live-runtime-missing-block")
                    } else if selectedMode == .live {
                        if let liveRuntimeConfiguration = executionService.liveRuntimeConfiguration {
                            VStack(alignment: .leading, spacing: 6) {
                                Label(
                                    "Live runtime: \(liveRuntimeConfiguration.summary)",
                                    systemImage: "bolt.horizontal.circle"
                                )
                                Label("Source: \(liveRuntimeConfiguration.sourceDescription)", systemImage: "server.rack")
                                Label("Safety: read-only workspace, no git/release side effects", systemImage: "lock.shield")
                                if let compiledPlan {
                                    let liveAgents = compiledPlan.agentBindings.values
                                        .sorted { $0.title < $1.title }
                                        .map { "\($0.title) (\($0.id))" }
                                    Label("Resolved live agents: \(liveAgents.count)", systemImage: "person.3.sequence")
                                    Text(liveAgents.joined(separator: ", "))
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                }
                            }
                            .font(.caption)
                            .foregroundStyle(.green)
                            .accessibilityIdentifier("live-runtime-config-block")
                        }
                    } else {
                        Label("Simulated mode uses the canonical local executor.", systemImage: "checkmark.circle")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            if let compiledPlan, selectedMode == .live {
                GroupBox {
                    DisclosureGroup(isExpanded: $showAdvancedOverrides) {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Optional per-profile provider/model/effort overrides for debugging or targeted experiments.")
                                .font(.caption)
                                .foregroundStyle(.secondary)

                            RunStartOverridesView(
                                plan: compiledPlan,
                                providerRegistry: providerRegistry,
                                startOptions: $startOptions
                            )
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .padding(.top, 6)
                    } label: {
                        VStack(alignment: .leading, spacing: 3) {
                            Text("Advanced Provider Overrides")
                                .font(.subheadline.bold())
                            Text("Usually leave this collapsed and use the catalog defaults.")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .accessibilityIdentifier("advanced-provider-overrides")
                }
            }

            GroupBox("Compilation Preview") {
                VStack(alignment: .leading, spacing: 8) {
                    switch compileState {
                    case .idle:
                        Text("Compile to validate `\(selectedWorkflow.relativePath)` and `agents.yaml` before starting.")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                    case .compiling:
                        HStack(spacing: 8) {
                            ProgressView()
                                .controlSize(.small)
                            Text("Compiling workflow...")
                                .font(.caption)
                        }

                    case .success(let stateCount, let agentCount):
                        HStack(spacing: 12) {
                            Label("\(stateCount) states", systemImage: "flowchart")
                            Label("\(agentCount) agents", systemImage: "person.3")
                        }
                        .font(.caption)
                        .foregroundStyle(.green)

                        if let compiledPlan {
                            VStack(alignment: .leading, spacing: 3) {
                                Text("Workflow: \(compiledPlan.workflowTitle)")
                                Text("Workflow ID: \(compiledPlan.workflowID)")
                                Text("Compiler version: \(compiledPlan.planCompilerVersion)")
                                Text("Workflow hash: \(compiledPlan.workflowSnapshotHash)")
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                                if selectedMode == .live {
                                    Text("Executor: Goose-backed live execution")
                                } else {
                                    Text("Executor: Simulated")
                                }
                            }
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                        }

                    case .error(let message):
                        Label(message, systemImage: "xmark.circle.fill")
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            GroupBox("Preflight") {
                VStack(alignment: .leading, spacing: 8) {
                    if let preflightReport {
                        HStack {
                            Label(preflightReport.status.rawValue.capitalized, systemImage: preflightIcon(preflightReport.status))
                                .foregroundStyle(preflightColor(preflightReport.status))
                            Spacer()
                            Button("Review Report") {
                                showPreflightSheet = true
                            }
                        }
                        .font(.caption)

                        Text("Configuration source: \(preflightReport.configurationSource.displayName)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)

                        if preflightReport.status == .warn && !requiresCleanPreflight {
                            Toggle("Allow start with warnings", isOn: $allowWarnStart)
                                .toggleStyle(.checkbox)
                                .font(.caption)
                        } else if preflightReport.status == .warn && requiresCleanPreflight {
                            Label("Run start is blocked until preflight is clean in current settings.", systemImage: "lock.fill")
                                .font(.caption)
                                .foregroundStyle(.orange)
                        }

                        if let firstBlockingIssue = preflightReport.blockingIssues.first {
                            Text(firstBlockingIssue)
                                .font(.caption2)
                                .foregroundStyle(.red)
                        } else if let firstWarning = preflightReport.warnings.first {
                            Text(firstWarning)
                                .font(.caption2)
                                .foregroundStyle(.orange)
                        }
                    } else {
                        Text("Preflight will run after compilation.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
                } // end ScrollView inner VStack
            } // end ScrollView

            Divider()

            HStack {
                Button("Cancel", role: .cancel) {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)

                Spacer()

                Button("Compile") {
                    compile()
                }
                .disabled(compileState == .compiling)

                Button("Start Run") {
                    startRun()
                }
                .buttonStyle(.borderedProminent)
                .disabled(
                    compiledPlan == nil
                    || isStarting
                    || compileState == .compiling
                    || liveModeRequiresConfiguration
                    || preflightBlocksStart
                    || (preflightReport?.status == .warn && requiresCleanPreflight)
                    || (warnRequiresConfirmation && allowWarnStart == false)
                )
                .accessibilityIdentifier("workflow-start-run-confirm-button")
            }
        }
        .padding(20)
        .frame(minWidth: 520, minHeight: 480)
        .task {
            resolveURLs()
            selectedMode = availableModes.contains(selectedMode) ? selectedMode : .simulated
            selectedWorkflow = availableWorkflows.first ?? .canonicalRelease
            compile()
        }
        .onChange(of: selectedMode) { _, newMode in
            if let firstWorkflow = WorkflowPreset.allCases.first(where: { $0.mode == newMode }) {
                selectedWorkflow = firstWorkflow
            }
            compile()
        }
        .onChange(of: selectedWorkflow) { _, _ in
            compile()
        }
        .onChange(of: startOptions) { _, _ in
            Task { await refreshPreflight() }
        }
        .sheet(isPresented: $showPreflightSheet) {
            if let preflightReport {
                NavigationStack {
                    PreflightReportView(report: preflightReport)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Done") { showPreflightSheet = false }
                            }
                        }
                }
                .frame(minWidth: 520, minHeight: 420)
            }
        }
    }

    private func resolveURLs() {
        workflowURLs = Dictionary(uniqueKeysWithValues: WorkflowPreset.allCases.compactMap { preset in
            let bundleURL = preset.bundleResourceName.flatMap { Bundle.main.url(forResource: $0, withExtension: "yaml") }
            let configuredWorkflowURL: URL? = {
                switch preset {
                case .canonicalRelease:
                    return URL(fileURLWithPath: appConfigurationStore.configuration.workflowSourcePath)
                case .proposalLoopLive:
                    return nil
                case .fullMVPLive:
                    return nil
                }
            }()
            guard let url = resolveExistingFile(at: [
                configuredWorkflowURL,
                URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent(preset.relativePath),
                URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Documents/Chainworks Forge/\(preset.relativePath)"),
                bundleURL
            ]) else {
                return nil
            }
            return (preset, url)
        })

        catalogURL = resolveExistingFile(at: [
            URL(fileURLWithPath: appConfigurationStore.configuration.agentCatalogSourcePath),
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("examples/agents/agents.yaml"),
            URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Documents/Chainworks Forge/examples/agents/agents.yaml"),
            Bundle.main.url(forResource: "agents", withExtension: "yaml")
        ])
    }

    private func resolveExistingFile(at candidates: [URL?]) -> URL? {
        for case let url? in candidates where FileManager.default.isReadableFile(atPath: url.path) {
            return url
        }
        return nil
    }

    private func compile() {
        guard let workflowURL = selectedWorkflowURL, let catalogURL else {
            compileState = .error("Unable to locate workflow or agent catalog")
            compiledPlan = nil
            preflightReport = nil
            return
        }

        compileState = .compiling

        do {
            let compiler = RunPlanCompiler(modelContext: modelContext)
            let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
            let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
            let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

            compiledPlan = plan
            compileState = .success(stateCount: plan.states.count, agentCount: plan.agentBindings.count)
            Task { await refreshPreflight() }
        } catch {
            compiledPlan = nil
            compileState = .error(error.localizedDescription)
            preflightReport = nil
        }
    }

    private func startRun() {
        guard let compiledPlan,
              let workflowURL = selectedWorkflowURL,
              let catalogURL,
              !preflightBlocksStart,
              !(preflightReport?.status == .warn && requiresCleanPreflight),
              !warnRequiresConfirmation || allowWarnStart else { return }
        isStarting = true

        do {
            let resolver = BackendProfileResolverV2(providerRegistry: providerRegistry)
            let providerBindings = try resolver.resolveBindings(plan: compiledPlan, startOptions: startOptions)
            let adjustedPlan = RunStartOverrideResolver.applying(bindings: providerBindings, to: compiledPlan)
            let compiler = RunPlanCompiler(modelContext: modelContext)
            let (run, workspace) = try compiler.createRun(
                for: idea,
                plan: adjustedPlan,
                workflowSourcePath: workflowURL.path,
                catalogSourcePath: catalogURL.path
            )
            run.providerBindingSnapshotJSON = encodeProviderBindings(providerBindings)
            run.startOptionsJSON = encodeStartOptions(startOptions)

            // Gap 3 (Proposal 007): Freeze DeliveryConfiguration for fullMVPLive preset
            if selectedWorkflow == .fullMVPLive {
                let repoRoot = FileManager.default.currentDirectoryPath
                let deliveryConfig = DeliveryConfiguration(
                    profileID: "dogfood_self",
                    profileLabel: "Self (Dogfood)",
                    sampleProfileID: nil,
                    repoIdentifier: URL(fileURLWithPath: repoRoot).lastPathComponent,
                    repoRoot: repoRoot,
                    baseBranch: "main",
                    worktreeBasePath: FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
                        .appendingPathComponent("Chainworks Forge/worktrees").path,
                    targetBranch: "release/\(run.id.uuidString.prefix(8))",
                    releaseTargetID: "sandbox_local",
                    releaseTargetLabel: "Local Sandbox",
                    releaseMode: .sandbox
                )
                run.deliveryConfigurationJSON = try? JSONEncoder().encode(deliveryConfig)
            }

            let preparedRun = PreparedRunStart(run: run, plan: adjustedPlan, workspace: workspace)
            onRunPrepared?(preparedRun)
            dismiss()
        } catch {
            compileState = .error("Failed to start run: \(error.localizedDescription)")
            isStarting = false
        }
    }

    private func refreshPreflight() async {
        guard let workflowURL = selectedWorkflowURL,
              let catalogURL else {
            preflightReport = nil
            return
        }
        let preflight = PreflightService(
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )
        preflightReport = await preflight.runReport(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            plan: compiledPlan,
            startOptions: startOptions
        )
        if preflightReport?.status != .warn {
            allowWarnStart = false
        }
    }

    private func encodeProviderBindings(_ bindings: [String: ResolvedProviderBinding]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(bindings)
    }

    private func encodeStartOptions(_ options: RunStartOptions) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(options)
    }

    private func preflightIcon(_ status: PreflightStatus) -> String {
        switch status {
        case .pass:
            return "checkmark.circle.fill"
        case .warn:
            return "exclamationmark.triangle.fill"
        case .fail:
            return "xmark.circle.fill"
        }
    }

    private func preflightColor(_ status: PreflightStatus) -> Color {
        switch status {
        case .pass:
            return .green
        case .warn:
            return .orange
        case .fail:
            return .red
        }
    }
}

// MARK: - WorkflowRunProgressView

struct WorkflowRunProgressView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService

    let run: Run

    @State private var selectedStage: StageExecution?
    @State private var selectedArtifact: Artifact?
    @State private var approvalComment = ""

    private var sortedStages: [StageExecution] {
        run.stageExecutions.sorted {
            if $0.startedAt == $1.startedAt {
                return $0.iteration < $1.iteration
            }
            return $0.startedAt < $1.startedAt
        }
    }

    private var activeAgents: [AgentExecution] {
        sortedStages.flatMap(\.agentExecutions).filter { $0.status == .running }
    }

    private var latestArtifacts: [Artifact] {
        let descriptor = FetchDescriptor<Artifact>(
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )

        let artifacts = ((try? modelContext.fetch(descriptor)) ?? [])
            .filter { $0.runID == run.id }
        return artifacts.sorted { lhs, rhs in
                if lhs.name == "final_feature_report" { return true }
                if rhs.name == "final_feature_report" { return false }
                return lhs.createdAt > rhs.createdAt
            }
    }

    private var orchestrator: WorkflowOrchestrator? {
        executionService.orchestrator(for: run.id)
    }

    private var liveTimeline: [LiveExecutionTimelineEntry] {
        orchestrator?.liveTimeline.reversed() ?? []
    }

    private var pendingApprovalRequest: ApprovalRequest? {
        executionService.pendingApprovals.values.first { $0.runID == run.id }
    }

    private var currentStageExecution: StageExecution? {
        sortedStages.last
    }

    private var approvalContextArtifacts: [Artifact] {
        let priority = [
            "proposal_revision_summary",
            "proposal_review_summary",
            "proposal_current",
            "proposal_review_po",
            "proposal_review_ux",
            "proposal_review_ui",
            "proposal_review_architect"
        ]
        var seen = Set<String>()
        let indexed = Dictionary(uniqueKeysWithValues: priority.enumerated().map { ($1, $0) })

        return latestArtifacts
            .filter { indexed[$0.name] != nil }
            .filter { seen.insert($0.name).inserted }
            .sorted { (lhs, rhs) in
                (indexed[lhs.name] ?? .max) < (indexed[rhs.name] ?? .max)
            }
    }

    private var latestDebugArtifacts: [Artifact] {
        var seen = Set<String>()
        return latestArtifacts
            .filter {
                $0.name.hasSuffix("_receipt.json")
                || $0.name.hasSuffix("_transcript.md")
            }
            .filter { seen.insert($0.name).inserted }
            .prefix(4)
            .map { $0 }
    }

    private var latestMeaningfulEvent: LiveExecutionTimelineEntry? {
        liveTimeline.first(where: { $0.event.type != .textChunk }) ?? liveTimeline.first
    }

    private var nextActionText: String {
        switch run.status {
        case .waitingApproval:
            return "Approve or reject the current proposal."
        case .blocked:
            return run.driftDetails ?? "Inspect the blocked stage and decide whether to resume."
        case .failed:
            return "Inspect receipts and artifacts, then retry or adjust the workflow."
        case .completed:
            return "Review the completed feature report and generated artifacts."
        case .running, .pending, .ready:
            return "Watch live progress and inspect artifacts as they arrive."
        case .cancelled:
            return "Run was cancelled."
        }
    }

    var body: some View {
        List {
            Section("Overview") {
                LabeledContent("Workflow", value: run.workflowTitle)
                LabeledContent("Status", value: run.status.rawValue)
                    .accessibilityIdentifier("run-status-\(run.status.rawValue)")
                LabeledContent("Current Stage", value: run.currentStageID ?? "None")
                LabeledContent("Elapsed", value: elapsedText)
                LabeledContent("Total Cost", value: run.totalCostCents.map { "\($0) cents" } ?? "Pending")
            }

            Section("Workflow Map") {
                WorkflowMapView(run: run)
            }

                Section("Current Phase") {
                LabeledContent("Phase", value: currentStageExecution?.label ?? run.currentStageID ?? "Not started")
                LabeledContent("Loop Iteration", value: currentStageExecution.map { "\($0.iteration)" } ?? "0")
                LabeledContent("Latest Event", value: latestMeaningfulEvent?.event.detail ?? "Waiting for the next execution event")
                if let sessionID = latestMeaningfulEvent?.event.sessionID {
                    LabeledContent("Session ID", value: sessionID)
                }
                LabeledContent("Next Action", value: nextActionText)
            }

            if let pendingApprovalRequest {
                // Gap 1 (Proposal 007): Show ReleaseGateView for manual_release approvals on delivery runs
                if run.deliveryConfigurationJSON != nil,
                   (pendingApprovalRequest.stageID.contains("manual_release") || pendingApprovalRequest.stageID.contains("state_11")) {
                    Section("Release Gate") {
                        ReleaseGateView(
                            run: run,
                            onApprove: {
                                executionService.resolveApproval(
                                    approvalID: pendingApprovalRequest.id,
                                    granted: true,
                                    comment: blankToNil(approvalComment)
                                )
                                approvalComment = ""
                            },
                            onReject: {
                                executionService.resolveApproval(
                                    approvalID: pendingApprovalRequest.id,
                                    granted: false,
                                    comment: blankToNil(approvalComment)
                                )
                                approvalComment = ""
                            }
                        )
                    }
                } else {
                Section("Approval Gate") {
                    Text("Run is waiting at \(pendingApprovalRequest.stageLabel).")
                        .font(.subheadline)
                    LabeledContent("Spend to Date", value: run.totalCostCents.map { "\($0) cents" } ?? "Pending")
                    if !approvalContextArtifacts.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Decision Context")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            ForEach(approvalContextArtifacts) { artifact in
                                Button {
                                    selectedArtifact = artifact
                                } label: {
                                    HStack {
                                        Text(artifact.name)
                                        Spacer()
                                        Text(artifact.format.rawValue)
                                            .font(.caption2)
                                            .foregroundStyle(.tertiary)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .accessibilityLabel("Open \(artifact.name)")
                                .accessibilityElement(children: .combine)
                                .accessibilityIdentifier("artifact-button-\(artifact.name)")
                            }
                        }
                    }
                    if !latestDebugArtifacts.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Receipts & Traces")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            ForEach(latestDebugArtifacts) { artifact in
                                Button {
                                    selectedArtifact = artifact
                                } label: {
                                    HStack {
                                        Text(artifact.name)
                                        Spacer()
                                        Text(artifact.format.rawValue)
                                            .font(.caption2)
                                            .foregroundStyle(.tertiary)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .accessibilityLabel("Open \(artifact.name)")
                                .accessibilityElement(children: .combine)
                                .accessibilityIdentifier("artifact-button-\(artifact.name)")
                            }
                        }
                    }
                    TextField("Comment", text: $approvalComment, axis: .vertical)
                        .textFieldStyle(.roundedBorder)
                HStack {
                        Button("Reject", role: .destructive) {
                            executionService.resolveApproval(
                                approvalID: pendingApprovalRequest.id,
                                granted: false,
                                comment: blankToNil(approvalComment)
                            )
                            approvalComment = ""
                        }
                        Spacer()
                        Button("Approve") {
                            executionService.resolveApproval(
                                approvalID: pendingApprovalRequest.id,
                                granted: true,
                                comment: blankToNil(approvalComment)
                            )
                            approvalComment = ""
                        }
                        .buttonStyle(.borderedProminent)
                    }
                }
                } // end else (generic approval gate)
            }

            Section("Stages") {
                if sortedStages.isEmpty {
                    Text("No stages have run yet.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(sortedStages) { stage in
                        Button {
                            selectedStage = stage
                        } label: {
                            HStack(alignment: .top, spacing: 12) {
                                Image(systemName: stageStatusIcon(stage.status))
                                    .foregroundStyle(stageStatusColor(stage.status))
                                    .frame(width: 18)
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack {
                                        Text(stage.label)
                                            .font(.headline)
                                        Spacer()
                                        Text(stage.status.rawValue)
                                            .font(.caption)
                                            .foregroundStyle(stageStatusColor(stage.status))
                                    }
                                    Text(stage.stageID)
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                    HStack(spacing: 10) {
                                        Text("Iteration \(stage.iteration)")
                                        Text("\(stage.agentExecutions.count) agent runs")
                                        if let completedAt = stage.completedAt {
                                            Text(durationString(from: stage.startedAt, to: completedAt))
                                        }
                                    }
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                }
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Section("Workflow Map") {
                WorkflowMapView(run: run)
                    .accessibilityIdentifier("workflow-map-section")
            }

            Section(run.status == .completed ? "Completed Feature Report" : "Artifacts") {
                if latestArtifacts.isEmpty {
                    Text("Artifacts will appear here as agent executions finish.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(latestArtifacts) { artifact in
                        Button {
                            selectedArtifact = artifact
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(artifact.name)
                                        .font(.headline)
                                    Text("\(artifact.stageID) · \(artifact.agentID)")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Text(artifact.format.rawValue)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Open \(artifact.name)")
                        .accessibilityElement(children: .combine)
                        .accessibilityIdentifier("artifact-button-\(artifact.name)")
                    }
                }
            }
        }
        .navigationTitle(run.workflowTitle)
        .accessibilityIdentifier("run-progress-view")
        .sheet(item: $selectedStage) { stage in
            WorkflowStageDetailView(stageExecution: stage, run: run)
        }
        .sheet(item: $selectedArtifact) { artifact in
            ArtifactInspectorView(artifact: artifact, run: run)
        }
    }

    private var elapsedText: String {
        durationString(from: run.startedAt, to: run.completedAt ?? Date())
    }

    private func durationString(from start: Date, to end: Date) -> String {
        let formatter = DateComponentsFormatter()
        let interval = max(0, end.timeIntervalSince(start))
        formatter.allowedUnits = interval >= 3600 ? [.hour, .minute] : [.minute, .second]
        formatter.unitsStyle = .abbreviated
        return formatter.string(from: interval) ?? "0s"
    }

    private func stageStatusIcon(_ status: StageStatus) -> String {
        switch status {
        case .pending, .ready: return "clock"
        case .running: return "bolt.circle.fill"
        case .waitingApproval: return "checkmark.seal.fill"
        case .blocked: return "pause.circle.fill"
        case .completed: return "checkmark.circle.fill"
        case .failed: return "xmark.circle.fill"
        case .skipped: return "arrow.uturn.forward.circle"
        }
    }

    private func stageStatusColor(_ status: StageStatus) -> Color {
        switch status {
        case .pending, .ready: return .gray
        case .running: return .green
        case .waitingApproval: return .orange
        case .blocked: return .yellow
        case .completed: return .green
        case .failed: return .red
        case .skipped: return .secondary
        }
    }

    private func blankToNil(_ value: String?) -> String? {
        guard let value else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

// MARK: - WorkflowStageDetailView

struct WorkflowStageDetailView: View {
    let stageExecution: StageExecution
    let run: Run

    @State private var selectedArtifact: Artifact?

    private var sortedAgentExecutions: [AgentExecution] {
        stageExecution.agentExecutions.sorted { $0.startedAt < $1.startedAt }
    }

    var body: some View {
        List {
            Section("Stage") {
                LabeledContent("Label", value: stageExecution.label)
                LabeledContent("Stage ID", value: stageExecution.stageID)
                LabeledContent("Status", value: stageExecution.status.rawValue)
                LabeledContent("Iteration", value: "\(stageExecution.iteration)")
                LabeledContent("Attempt", value: "\(stageExecution.attemptNumber)")
            }

            Section("Agent Executions") {
                if sortedAgentExecutions.isEmpty {
                    Text("No agent executions recorded for this stage.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(sortedAgentExecutions) { execution in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(execution.agentTitle)
                                    .font(.headline)
                                Spacer()
                                Text(execution.status.rawValue)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Text(execution.taskName)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            agentMetadataRow(for: execution)
                            if let logSnippet = execution.logSnippet, !logSnippet.isEmpty {
                                Text(logSnippet)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                            if !execution.artifacts.isEmpty {
                                ScrollView(.horizontal, showsIndicators: false) {
                                    HStack(spacing: 8) {
                                        ForEach(execution.artifacts.sorted { $0.createdAt > $1.createdAt }) { artifact in
                                            Button(artifact.name) {
                                                selectedArtifact = artifact
                                            }
                                            .buttonStyle(.bordered)
                                            .controlSize(.small)
                                        }
                                    }
                                }
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }
            }
        }
        .frame(minWidth: 560, minHeight: 420)
        .accessibilityIdentifier("stage-detail-view")
        .sheet(item: $selectedArtifact) { artifact in
            ArtifactInspectorView(artifact: artifact, run: run)
        }
    }

    @ViewBuilder
    private func agentMetadataRow(for execution: AgentExecution) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 10) {
                Text(execution.provider)
                if let model = execution.resolvedModel, !model.isEmpty {
                    Text(model)
                }
                Text(execution.effort)
            }
            .font(.caption2)
            .foregroundStyle(.secondary)

            HStack(spacing: 10) {
                Text(durationString(from: execution.startedAt, to: execution.completedAt ?? Date()))
                if let cost = execution.costCents {
                    Text("\(cost) cents")
                }
                if let adapterVersion = execution.adapterVersion, !adapterVersion.isEmpty {
                    Text(adapterVersion)
                }
            }
            .font(.caption2)
            .foregroundStyle(.tertiary)

            if receiptArtifact(for: execution) != nil {
                Button("Open Provider Receipt") {
                    selectedArtifact = receiptArtifact(for: execution)
                }
                .buttonStyle(.borderless)
                .font(.caption2)
            }
        }
    }

    private func receiptArtifact(for execution: AgentExecution) -> Artifact? {
        execution.artifacts.first { $0.name.hasSuffix("_receipt.json") }
            ?? execution.artifacts.first { $0.contractID == "provider_receipt" }
    }

    private func durationString(from start: Date, to end: Date) -> String {
        let formatter = DateComponentsFormatter()
        let interval = max(0, end.timeIntervalSince(start))
        formatter.allowedUnits = interval >= 3600 ? [.hour, .minute] : [.minute, .second]
        formatter.unitsStyle = .abbreviated
        return formatter.string(from: interval) ?? "0s"
    }
}

// MARK: - WorkflowArtifactInspectorView

struct WorkflowArtifactInspectorView: View {
    @Environment(\.modelContext) private var modelContext

    let run: Run
    let artifact: Artifact

    @State private var renderedContent = "Loading artifact..."

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(artifact.name)
                    .font(.headline)
                    .accessibilityIdentifier("artifact-inspector-title")
                Text("\(artifact.stageID) · \(artifact.agentID) · \(artifact.format.rawValue)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text(artifact.filePath)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .textSelection(.enabled)
            }

            Divider()

            ScrollView {
                if artifact.format == .markdown {
                    // Render markdown with full block-level support (proposal contract REQ-010)
                    let attributed = (try? AttributedString(markdown: renderedContent,
                                                            options: .init(interpretedSyntax: .full))) ?? AttributedString(renderedContent)
                    Text(attributed)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                } else if artifact.format == .diff {
                    // Diff: monospaced with color per line
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(renderedContent.components(separatedBy: "\n").enumerated()), id: \.offset) { _, line in
                            Text(line)
                                .font(.system(.caption, design: .monospaced))
                                .foregroundStyle(line.hasPrefix("+") ? .green : line.hasPrefix("-") ? .red : line.hasPrefix("@@") ? .blue : .primary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                    .textSelection(.enabled)
                } else {
                    // JSON / report: monospaced
                    Text(renderedContent)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                        .font(.system(.body, design: .monospaced))
                }
            }
            .accessibilityIdentifier("artifact-inspector-content")
        }
        .padding()
        .frame(minWidth: 640, minHeight: 480)
        .accessibilityIdentifier("artifact-inspector-view")
        .task(id: artifact.id) {
            renderedContent = loadContent()
        }
    }

    private func loadContent() -> String {
        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: URL(fileURLWithPath: run.workspaceRoot),
            artifactRoot: URL(fileURLWithPath: run.artifactRoot),
            worktreeRoot: nil
        )

        do {
            let manager = ArtifactManager(modelContext: modelContext)
            let data = try manager.readArtifact(artifact, workspace: workspace)

            if artifact.format == .json,
               let jsonObject = try? JSONSerialization.jsonObject(with: data),
               let prettyData = try? JSONSerialization.data(withJSONObject: jsonObject, options: [.prettyPrinted]),
               let string = String(data: prettyData, encoding: .utf8) {
                return string
            }

            if let string = String(data: data, encoding: .utf8) {
                return string
            }

            return "Binary artifact (\(data.count) bytes)"
        } catch {
            return "Failed to load artifact: \(error.localizedDescription)"
        }
    }
}
#endif

#Preview("Ideas — Operator List") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

    return IdeaListView()
        .modelContainer(container)
        .environment(executionService)
        .frame(width: 1280, height: 820)
}

#Preview("Start New Run — Live") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
    let idea = Idea(
        title: "Investigate provider setup",
        body: "Make Codex and Claude configuration legible and Goose-backed.",
        attachmentPath: "/Users/user/Documents/specs/provider-setup.md"
    )

    return WorkflowStartRunSheet(idea: idea)
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .frame(width: 560, height: 860)
}
