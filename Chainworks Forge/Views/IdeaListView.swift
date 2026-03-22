import SwiftUI
import SwiftData

struct IdeaListView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Query(sort: \Idea.createdAt, order: .reverse) private var ideas: [Idea]
    @State private var newTitle = ""
    @State private var newBody = ""
    @State private var newAttachmentPath = ""

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                // Summary strip (UI-001)
                summaryStrip

                Group {
                    if ideas.isEmpty {
                        ContentUnavailableView(
                            "No ideas yet",
                            systemImage: "lightbulb",
                            description: Text("Create your first idea to get started.")
                        )
                    } else {
                        List {
                            ForEach(ideas) { idea in
                                NavigationLink {
                                    IdeaDetailView(idea: idea)
                                } label: {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(idea.title).font(.headline)
                                        HStack {
                                            Text(idea.status.rawValue.capitalized)
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
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
                            }
                            .onDelete(perform: deleteIdeas)
                        }
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
                    Button(action: { showNewIdeaSheet = true }) {
                        Label("New Idea", systemImage: "plus")
                    }
                }
            }
            .sheet(isPresented: $showNewIdeaSheet) {
                newIdeaSheet
            }
        } detail: {
            Text("Select an idea")
                .foregroundStyle(.secondary)
        }
    }

    @State private var showNewIdeaSheet = false

    // MARK: - Summary Strip

    private var summaryStrip: some View {
        let draftCount = ideas.filter { $0.status == .draft }.count
        let activeCount = ideas.filter { $0.status == .active }.count

        return HStack {
            Image(systemName: "lightbulb.fill")
                .foregroundStyle(.blue)
            Text("\(ideas.count) ideas \u{00B7} \(draftCount) drafts \u{00B7} \(activeCount) active")
            Spacer()
            if executionService.hasActiveRuns {
                Label("\(executionService.activeOrchestrators.count) running", systemImage: "bolt.fill")
                    .font(.caption)
                    .foregroundStyle(.green)
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
        resetForm()
    }

    private func resetForm() {
        newTitle = ""
        newBody = ""
        newAttachmentPath = ""
    }

    private func deleteIdeas(offsets: IndexSet) {
        for index in offsets {
            modelContext.delete(ideas[index])
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

    /// Whether this idea has an active run (prevents starting another).
    private var hasActiveRun: Bool {
        idea.runs.contains { [.pending, .ready, .running, .waitingApproval, .blocked].contains($0.status) }
    }

    var body: some View {
        Form {
            Section("Idea") {
                LabeledContent("Title", value: idea.title)
                LabeledContent("Status", value: idea.status.rawValue.capitalized)
                LabeledContent("Created", value: idea.createdAt, format: .dateTime)
                if let path = idea.attachmentPath {
                    LabeledContent("Attachment", value: path)
                }
            }
            Section("Body") {
                Text(idea.body)
                    .textSelection(.enabled)
            }

            // Proposal 002 + 004: Start New Run action
            Section {
                Button {
                    showStartRunSheet = true
                } label: {
                    Label("Start New Run", systemImage: "play.fill")
                }
                .disabled(hasActiveRun)
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("start-new-run-button")

                if hasActiveRun {
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
                        NavigationLink {
                            WorkflowRunProgressView(run: run)
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
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(idea.title)
        .sheet(isPresented: $showStartRunSheet) {
            WorkflowStartRunSheet(idea: idea)
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
struct WorkflowStartRunSheet: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(\.dismiss) private var dismiss
    @Environment(ExecutionService.self) private var executionService

    let idea: Idea

    @State private var compileState: CompileState = .idle
    @State private var compiledPlan: RunPlan?
    @State private var workflowURL: URL?
    @State private var catalogURL: URL?
    @State private var isStarting = false

    private enum CompileState: Equatable {
        case idle
        case compiling
        case success(stateCount: Int, agentCount: Int)
        case error(String)
    }

    var body: some View {
        VStack(spacing: 16) {
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

            GroupBox("Compilation Preview") {
                VStack(alignment: .leading, spacing: 8) {
                    switch compileState {
                    case .idle:
                        Text("Compile to validate `workflow.yaml` and `agents.yaml` before starting.")
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

            Spacer()

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
                .disabled(compiledPlan == nil || isStarting || compileState == .compiling)
                .accessibilityIdentifier("workflow-start-run-confirm-button")
            }
        }
        .padding(20)
        .frame(minWidth: 520, minHeight: 400)
        .task {
            resolveURLs()
            compile()
        }
    }

    private func resolveURLs() {
        workflowURL = resolveExistingFile(at: [
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("examples/workflows/workflow.yaml"),
            URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Documents/Chainworks Forge/examples/workflows/workflow.yaml"),
            Bundle.main.url(forResource: "workflow", withExtension: "yaml")
        ])

        catalogURL = resolveExistingFile(at: [
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
        guard let workflowURL, let catalogURL else {
            compileState = .error("Unable to locate workflow or agent catalog")
            compiledPlan = nil
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
        } catch {
            compiledPlan = nil
            compileState = .error(error.localizedDescription)
        }
    }

    private func startRun() {
        guard let compiledPlan else { return }
        isStarting = true

        do {
            let compiler = RunPlanCompiler(modelContext: modelContext)
            let (run, workspace) = try compiler.createRun(
                for: idea,
                plan: compiledPlan,
                workflowSourcePath: workflowURL?.path ?? "",
                catalogSourcePath: catalogURL?.path ?? ""
            )
            idea.status = .active
            executionService.startRun(run: run, plan: compiledPlan, workspace: workspace)
            dismiss()
        } catch {
            compileState = .error("Failed to start run: \(error.localizedDescription)")
            isStarting = false
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
        sortedStages
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)
            .sorted { $0.createdAt > $1.createdAt }
    }

    private var pendingApprovalRequest: ApprovalRequest? {
        executionService.pendingApprovals.values.first { $0.runID == run.id }
    }

    var body: some View {
        List {
            Section("Overview") {
                LabeledContent("Workflow", value: run.workflowTitle)
                LabeledContent("Status", value: run.status.rawValue)
                LabeledContent("Current Stage", value: run.currentStageID ?? "None")
                LabeledContent("Elapsed", value: elapsedText)
                LabeledContent("Total Cost", value: run.totalCostCents.map { "\($0) cents" } ?? "Pending")
            }

            if let pendingApprovalRequest {
                Section("Approval Gate") {
                    Text("Run is waiting at \(pendingApprovalRequest.stageLabel).")
                        .font(.subheadline)
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

            Section("Active Agents") {
                if activeAgents.isEmpty {
                    Text(run.status == .completed ? "No active agents." : "No agents are currently executing.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(activeAgents) { agentExecution in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(agentExecution.agentTitle)
                                .font(.headline)
                            Text(agentExecution.taskName)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            if let logSnippet = blankToNil(agentExecution.logSnippet) {
                                Text(logSnippet)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                    }
                }
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
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .navigationTitle(run.workflowTitle)
        .sheet(item: $selectedStage) { stage in
            WorkflowStageDetailView(stageExecution: stage, run: run)
        }
        .sheet(item: $selectedArtifact) { artifact in
            WorkflowArtifactInspectorView(run: run, artifact: artifact)
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
        .sheet(item: $selectedArtifact) { artifact in
            WorkflowArtifactInspectorView(run: run, artifact: artifact)
        }
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
                Text(renderedContent)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
                    .font(.system(.body, design: .monospaced))
            }
        }
        .padding()
        .frame(minWidth: 640, minHeight: 480)
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
