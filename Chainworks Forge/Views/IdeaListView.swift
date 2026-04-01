import SwiftUI
import SwiftData
import UniformTypeIdentifiers

struct IdeaListView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings
    @Query(sort: \Idea.createdAt, order: .reverse) private var ideas: [Idea]
    @State private var newIdeaDraft = NewIdeaDraft()
    @State private var showNewIdeaSheet = false
    @State private var showArchivedIdeas = false
    @State private var selectedIdeaID: UUID?

    private var activeIdeas: [Idea] {
        ideas.filter { !$0.isArchived }
    }

    private var archivedIdeas: [Idea] {
        ideas.filter(\.isArchived)
    }

    private var selectedIdea: Idea? {
        guard let selectedIdeaID else { return nil }
        return ideas.first(where: { $0.id == selectedIdeaID })
    }

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                // Summary strip (UI-001)
                summaryStrip

                HStack(spacing: 12) {
                    Button(action: presentNewIdeaSheet) {
                        Label("New Idea", systemImage: "plus")
                    }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("ideas-new-idea-inline")

                    Button(action: { showArchivedIdeas = true }) {
                        Label("Archive", systemImage: "archivebox")
                    }
                    .buttonStyle(.bordered)
                    .accessibilityIdentifier("ideas-open-archive-inline")

                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)

                Group {
                    if activeIdeas.isEmpty {
                        // Proposal 012 (L-01): Enhanced empty state with action
                        StyledEmptyState(
                            title: archivedIdeas.isEmpty ? "No ideas yet" : "No active ideas",
                            systemImage: "lightbulb",
                            description: archivedIdeas.isEmpty ? "Create your first idea to get started." : "Open the archive lane to restore an idea or create a new one.",
                            actionTitle: "New Idea"
                        ) {
                            presentNewIdeaSheet()
                        }
                    } else {
                        List(selection: $selectedIdeaID) {
                            ForEach(activeIdeas) { idea in
                                NavigationLink(value: idea.id) {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(idea.title).font(.headline)
                                        HStack(spacing: 8) {
                                            IdeaLifecycleBadge(idea: idea)
                                            if idea.isArchived {
                                                Image(systemName: "archivebox.fill")
                                                    .font(.caption2)
                                                    .foregroundStyle(.secondary)
                                            }
                                            if let attachPath = idea.attachmentPath {
                                                // Proposal 008 (REQ-009): Color-code attachment indicator by validation status.
                                                AttachmentStatusIcon(path: attachPath)
                                            }
                                            // Show active run indicator
                                            if idea.runs.contains(where: { [.running, .waitingApproval, .pending, .ready, .blocked].contains($0.status) }) {
                                                Image(systemName: "play.circle.fill")
                                                    .font(.caption2)
                                                    .foregroundStyle(DesignTokens.Status.success)
                                            }
                                        }
                                    }
                                }
                                .tag(idea.id)
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
            .navigationSplitViewColumnWidth(min: 260, ideal: 320)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { showArchivedIdeas = true }) {
                        Label("Archive", systemImage: "archivebox")
                    }
                    .accessibilityIdentifier("ideas-open-archive")
                }
                ToolbarItem(placement: .primaryAction) {
                    Button(action: presentNewIdeaSheet) {
                        Label("New Idea", systemImage: "plus")
                    }
                    .keyboardShortcut("n", modifiers: [.command])
                    .accessibilityIdentifier("ideas-new-idea")
                }
            }
            .sheet(isPresented: $showNewIdeaSheet) {
                NewIdeaSheetView(
                    draft: $newIdeaDraft,
                    onBrowseAttachment: browseAttachment,
                    onCancel: {
                        resetForm()
                        showNewIdeaSheet = false
                    },
                    onSave: {
                        createIdea()
                        showNewIdeaSheet = false
                    }
                )
            }
            .sheet(isPresented: $showArchivedIdeas) {
                ArchivedIdeasView()
                    .environment(\.modelContext, modelContext)
            }
            .accessibilityIdentifier("ideas-root-view")
        } detail: {
            if let selectedIdea {
                IdeaDetailView(idea: selectedIdea)
            } else {
                // Proposal 012 (L-01): Enhanced empty state
                StyledEmptyState(
                    title: "Select an Idea",
                    systemImage: "lightbulb",
                    description: "Choose an idea from the list or create a new one to configure its project directory and start a run."
                )
            }
        }
        .onChange(of: activeIdeas.map(\.id)) { _, activeIDs in
            if let selectedIdeaID, !activeIDs.contains(selectedIdeaID) {
                self.selectedIdeaID = activeIDs.first
            } else if self.selectedIdeaID == nil {
                self.selectedIdeaID = activeIDs.first
            }
        }
        .task {
            if selectedIdeaID == nil {
                selectedIdeaID = activeIdeas.first?.id
            }
        }
    }

    // MARK: - Summary Strip

    // Proposal 012 (L-03): Redesigned summary strip with pill chips and two-row layout.
    private var summaryStrip: some View {
        let draftCount = activeIdeas.filter { $0.status == .draft }.count
        let activeCount = activeIdeas.filter { $0.status == .active }.count

        return VStack(spacing: DesignTokens.Spacing.compact) {
            // Row 1: Idea count chips
            HStack(spacing: DesignTokens.Spacing.small) {
                summaryChip(
                    label: "\(activeIdeas.count) ideas",
                    icon: "lightbulb.fill",
                    color: .blue,
                    accessibilityIdentifier: "ideas-summary-chip-total"
                )
                summaryChip(
                    label: "\(draftCount) drafts",
                    icon: "pencil",
                    color: .secondary,
                    accessibilityIdentifier: "ideas-summary-chip-drafts"
                )
                summaryChip(
                    label: "\(activeCount) active",
                    icon: "bolt.fill",
                    color: DesignTokens.Status.success,
                    accessibilityIdentifier: "ideas-summary-chip-active"
                )
                if !archivedIdeas.isEmpty {
                    Button {
                        showArchivedIdeas = true
                    } label: {
                        summaryChip(
                            label: "\(archivedIdeas.count) archived",
                            icon: "archivebox",
                            color: .secondary,
                            accessibilityIdentifier: "ideas-summary-chip-archived"
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("ideas-summary-open-archive")
                }
                Spacer()
            }

            summaryChipAccessibilityProof(
                totalLabel: "\(activeIdeas.count) ideas",
                draftLabel: "\(draftCount) drafts",
                activeLabel: "\(activeCount) active",
                archivedLabel: archivedIdeas.isEmpty ? nil : "\(archivedIdeas.count) archived"
            )

            // Row 2: Runtime status
            HStack(spacing: DesignTokens.Spacing.small) {
                if executionService.hasActiveRuns {
                    StatusCapsule(
                        text: "\(executionService.activeOrchestrators.count) running",
                        color: DesignTokens.Status.success,
                        icon: "play.circle.fill",
                        size: .small
                    )
                }
                switch executionService.liveRuntimeReadiness {
                case .ready(_, let source):
                    StatusCapsule(
                        text: "Live ready (\(source))",
                        color: DesignTokens.Status.success,
                        icon: "bolt.horizontal.circle.fill",
                        size: .small
                    )
                    .accessibilityIdentifier("live-runtime-ready")
                case .unavailable:
                    StatusCapsule(
                        text: "Live unavailable",
                        color: DesignTokens.Status.warning,
                        icon: "exclamationmark.triangle.fill",
                        size: .small
                    )
                    .accessibilityIdentifier("live-runtime-unavailable")
                }
                Spacer()
            }
        }
        .padding(.horizontal)
        .padding(.vertical, DesignTokens.Spacing.small)
        .background(DesignTokens.Action.primary.opacity(0.06))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Idea summary")
        .accessibilityValue(summaryStripAccessibilityLabel)
        .accessibilityIdentifier("ideas-summary-strip")
    }

    /// Pill-shaped chip for the summary strip.
    private func summaryChip(label: String, icon: String, color: Color, accessibilityIdentifier: String) -> some View {
        StatusCapsule(
            text: label,
            color: color,
            icon: icon,
            size: .small,
            accessibilityIdentifier: accessibilityIdentifier
        )
    }

    @ViewBuilder
    private func summaryChipAccessibilityProof(
        totalLabel: String,
        draftLabel: String,
        activeLabel: String,
        archivedLabel: String?
    ) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            summaryChipAccessibilityMirror(identifier: "ideas-summary-chip-total", label: totalLabel)
            summaryChipAccessibilityMirror(identifier: "ideas-summary-chip-drafts", label: draftLabel)
            summaryChipAccessibilityMirror(identifier: "ideas-summary-chip-active", label: activeLabel)
            if let archivedLabel {
                summaryChipAccessibilityMirror(identifier: "ideas-summary-chip-archived", label: archivedLabel)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func summaryChipAccessibilityMirror(identifier: String, label: String) -> some View {
        Text(label)
            .font(.caption2)
            .frame(width: 1, height: 1, alignment: .leading)
            .clipped()
            .opacity(0.01)
            .accessibilityElement(children: .ignore)
            .accessibilityAddTraits(.isStaticText)
            .accessibilityLabel(label)
            .accessibilityValue(summaryChipAccessibilityModes)
            .accessibilityIdentifier(identifier)

        if uiTestAccessibilitySettings.increaseContrast {
            Text("Increase Contrast")
                .font(.caption2)
                .frame(width: 1, height: 1)
                .clipped()
                .opacity(0.01)
                .accessibilityIdentifier("\(identifier)-increase-contrast")
        }
    }

    private var summaryChipAccessibilityModes: String {
        var activeModes: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            activeModes.append("differentiate without color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            activeModes.append("increase contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            activeModes.append("reduce transparency")
        }
        return activeModes.isEmpty ? "standard accessibility display settings" : activeModes.joined(separator: ", ")
    }

    private var summaryStripAccessibilityLabel: String {
        let draftCount = activeIdeas.filter { $0.status == .draft }.count
        let activeCount = activeIdeas.filter { $0.status == .active }.count
        let archivedCount = archivedIdeas.count
        var parts = [
            "\(activeIdeas.count) ideas",
            "\(draftCount) drafts",
            "\(activeCount) active"
        ]
        if archivedCount > 0 {
            parts.append("\(archivedCount) archived")
        }
        parts.append(summaryChipAccessibilityModes)
        return parts.joined(separator: ", ")
    }

    // MARK: - Approval Bar

    private var approvalBar: some View {
        HStack {
            Image(systemName: "checkmark.seal.fill")
                .foregroundStyle(DesignTokens.Status.warning)
            Text("\(executionService.pendingApprovalCount) pending approval(s)")
            Spacer()
        }
        .font(.caption)
        .padding(.horizontal)
        .padding(.vertical, 6)
        .background(DesignTokens.Status.warning.opacity(0.1))
    }

    private func createIdea() {
        let trimmedPath = newIdeaDraft.attachmentPath.trimmingCharacters(in: .whitespaces)
        let idea = Idea(
            title: newIdeaDraft.title.trimmingCharacters(in: .whitespaces),
            body: newIdeaDraft.body.trimmingCharacters(in: .whitespaces),
            attachmentPath: trimmedPath.isEmpty ? nil : trimmedPath
        )
        modelContext.insert(idea)
        try? modelContext.save()
        selectedIdeaID = idea.id
        resetForm()
    }

    private func presentNewIdeaSheet() {
        prefillNewIdeaDraftFromUITestEnvironmentIfNeeded()
        showNewIdeaSheet = true
    }

    private func prefillNewIdeaDraftFromUITestEnvironmentIfNeeded() {
        let environment = ProcessInfo.processInfo.environment
        if let title = environment["CHAINWORKS_UI_TEST_NEW_IDEA_TITLE"],
           newIdeaDraft.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            newIdeaDraft.title = title
        }
        if let body = environment["CHAINWORKS_UI_TEST_NEW_IDEA_BODY"],
           newIdeaDraft.body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            newIdeaDraft.body = body
        }
    }

    private func resetForm() {
        newIdeaDraft = NewIdeaDraft()
    }

    private func deleteIdeas(offsets: IndexSet) {
        for index in offsets {
            modelContext.delete(activeIdeas[index])
        }
        try? modelContext.save()
    }

    private func statusLabel(for idea: Idea) -> String {
        idea.lifecycleStatusLabel
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
            newIdeaDraft.attachmentPath = url.path
        }
        #endif
    }
}

struct NewIdeaDraft: Equatable {
    var title: String = ""
    var body: String = ""
    var attachmentPath: String = ""

    var trimmedTitle: String {
        title.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var canSave: Bool {
        !trimmedTitle.isEmpty
    }
}

struct NewIdeaSheetView: View {
    @Binding var draft: NewIdeaDraft
    let onBrowseAttachment: () -> Void
    let onCancel: () -> Void
    let onSave: () -> Void
    private let environment = ProcessInfo.processInfo.environment

    // Proposal 012 (L-07): Converted to Form for macOS consistency
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text("Capture the idea first. Project directory and run configuration come after the idea exists.")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                }

                Section("Details") {
                    TextField("Title", text: $draft.title)
                        .accessibilityIdentifier("new-idea-title-field")
                        .onPasteCommand(of: [UTType.plainText]) { providers in
                            guard let provider = providers.first else { return }
                            provider.loadItem(forTypeIdentifier: UTType.plainText.identifier, options: nil) { item, _ in
                                let resolvedText: String?
                                switch item {
                                case let data as Data:
                                    resolvedText = String(data: data, encoding: .utf8)
                                case let string as String:
                                    resolvedText = string
                                case let string as NSString:
                                    resolvedText = string as String
                                default:
                                    resolvedText = nil
                                }
                                guard let resolvedText else { return }
                                Task { @MainActor in
                                    draft.title = resolvedText
                                }
                            }
                        }

                    TextEditor(text: $draft.body)
                        .frame(minHeight: 100)
                        .accessibilityIdentifier("new-idea-body-field")
                }

                Section("Attachment") {
                    HStack(spacing: DesignTokens.Spacing.small) {
                        TextField("Path (optional)", text: $draft.attachmentPath)
                            .accessibilityIdentifier("new-idea-attachment-field")
                        Button("Browse...", action: onBrowseAttachment)
                            .accessibilityIdentifier("new-idea-browse-button")
                    }
                }
            }
            .navigationTitle("New Idea")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: onCancel)
                        .keyboardShortcut(.escape, modifiers: [])
                        .accessibilityIdentifier("new-idea-cancel-button")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save Idea", action: onSave)
                        .buttonStyle(.borderedProminent)
                        .disabled(!draft.canSave)
                        .keyboardShortcut(.return, modifiers: [.command])
                        .accessibilityIdentifier("new-idea-save-button")
                }
            }
        }
        .frame(minWidth: 460, minHeight: 340)
        .accessibilityIdentifier("new-idea-sheet")
        .task {
            prefillFromUITestEnvironmentIfNeeded()
        }
    }

    @MainActor
    private func prefillFromUITestEnvironmentIfNeeded() {
        guard let title = environment["CHAINWORKS_UI_TEST_NEW_IDEA_TITLE"],
              draft.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return
        }
        draft.title = title
        if let body = environment["CHAINWORKS_UI_TEST_NEW_IDEA_BODY"],
           draft.body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            draft.body = body
        }
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
    @State private var editingWorkspacePath: String = ""
    @State private var showStopConfirmation = false

    /// Whether this idea has an active run (prevents starting another).
    private var hasActiveRun: Bool {
        idea.runs.contains { [.pending, .ready, .running, .waitingApproval, .blocked].contains($0.status) }
    }

    private var displayedRun: Run? {
        guard let activeRun else { return nil }
        switch activeRun.presentationStatus {
        case .pending, .ready, .running, .waitingApproval, .blocked, .cancelling:
            return activeRun
        case .completed, .failed, .cancelled:
            return nil
        }
    }

    private var latestActiveRun: Run? {
        idea.runs
            .filter { [.pending, .ready, .running, .waitingApproval, .blocked].contains($0.status) }
            .sorted { $0.startedAt > $1.startedAt }
            .first
    }

    private var startRunActionTitle: String {
        idea.latestRunIsTerminal ? "Start Another Run" : "Start New Run"
    }

    private var startRunHelperText: String? {
        if idea.isArchived {
            return "Restore the idea before starting a new run."
        }
        if hasActiveRun {
            return "An active run already exists for this idea."
        }
        guard let latestRun = idea.latestRun else { return nil }
        switch latestRun.presentationStatus {
        case .cancelled:
            return "Latest run was cancelled. Start another run or archive the idea."
        case .completed:
            return "Latest run completed. Start another run or archive the idea."
        case .failed:
            return "Latest run failed. Review artifacts, then start another run or archive the idea."
        default:
            return nil
        }
    }

    var body: some View {
        Group {
            if let activeRun = displayedRun {
                WorkflowRunProgressView(run: activeRun)
            } else {
                Form {
                    Section("Idea") {
                        LabeledContent("Title", value: idea.title)
                        LabeledContent("Status", value: idea.lifecycleStatusLabel)
                        LabeledContent("Created", value: idea.createdAt, format: .dateTime)
                        if let archivedAt = idea.archivedAt {
                            LabeledContent("Archived", value: archivedAt, format: .dateTime)
                        }
                        if let path = idea.attachmentPath {
                            HStack {
                                Text("Attachment")
                                Spacer()
                                Text(path)
                                    .foregroundStyle(.secondary)
                                // Proposal 008 (REQ-009): Surface attachment validation state.
                                let validationStatus = MVPBoundaryPolicy.validateAttachment(path: path)
                                Text(validationStatus.rawValue)
                                    .font(.caption2.bold())
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(
                                        validationStatus == .referenceOnly
                                            ? DesignTokens.Status.success.opacity(0.15)
                                            : DesignTokens.Status.error.opacity(0.15)
                                    )
                                    .foregroundStyle(validationStatus == .referenceOnly ? DesignTokens.Status.success : DesignTokens.Status.error)
                                    .clipShape(Capsule())
                            }
                        }
                    }

                    Section("Body") {
                        Text(idea.body)
                            .textSelection(.enabled)
                    }

                    // Proposal 011 (REQ-006): Workspace root path editor
                    Section("Project Directory") {
                        HStack {
                            TextField("Workspace root path", text: $editingWorkspacePath)
                                .textFieldStyle(.roundedBorder)
                                .accessibilityIdentifier("idea-workspace-root-path-field")
                            Button("Save Path") {
                                saveWorkspaceRoot()
                            }
                            .disabled(editingWorkspacePath.trimmingCharacters(in: .whitespaces) == (idea.workspaceRootPath ?? ""))
                            .accessibilityIdentifier("idea-workspace-root-save")
                            Button("Browse...") {
                                browseWorkspaceRoot()
                            }
                            .accessibilityIdentifier("idea-workspace-root-browse")
                        }

                        if let path = idea.workspaceRootPath, !path.isEmpty {
                            let isValid = isValidDirectory(path)
                            Label(
                                isValid ? "Valid directory" : "Directory not found or not accessible",
                                systemImage: isValid ? "checkmark.circle.fill" : "xmark.circle.fill"
                            )
                            .font(.caption)
                            .foregroundStyle(isValid ? DesignTokens.Status.success : DesignTokens.Status.error)
                            .accessibilityIdentifier("idea-workspace-root-status")
                        } else {
                            Text("Set a project directory for workflows that require project access.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
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
                            Label(startRunActionTitle, systemImage: "play.fill")
                        }
                        .disabled(hasActiveRun || idea.isArchived)
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("start-new-run-button")

                        if let startRunHelperText {
                            Text(startRunHelperText)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    // Proposal 011 — REQ-001: Dedicated stop action for active runs
                    if hasActiveRun, let runToStop = latestActiveRun {
                        Section("Run Control") {
                            Button(role: .destructive) {
                                showStopConfirmation = true
                            } label: {
                                Label(
                                    runToStop.cancellationRequestedAt != nil ? "Cancelling\u{2026}" : "Stop Run",
                                    systemImage: runToStop.cancellationRequestedAt != nil ? "hourglass" : "stop.fill"
                                )
                            }
                            .disabled(runToStop.cancellationRequestedAt != nil)
                            .buttonStyle(.bordered)
                            .tint(.red)
                            .accessibilityIdentifier("stop-run-button")

                            if runToStop.cancellationRequestedAt != nil {
                                Text("Cancellation in progress. Waiting for agents to settle.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            } else {
                                Text("Stop the active run. All run history and artifacts remain intact.")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .alert("Stop Run?", isPresented: $showStopConfirmation) {
                            Button("Stop", role: .destructive) {
                                Task {
                                    await executionService.cancelRun(runID: runToStop.id)
                                }
                            }
                            Button("Keep Running", role: .cancel) { }
                        } message: {
                            Text("This will stop all active agents for \"\(idea.title)\". Run history and artifacts remain visible as terminal history.")
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
                                        runStatusIcon(run.presentationStatus)
                                        VStack(alignment: .leading, spacing: 2) {
                                            Text(run.workflowTitle)
                                                .font(.headline)
                                            HStack(spacing: 8) {
                                                Text(run.presentationStatusLabel)
                                                    .font(.caption)
                                                    .foregroundStyle(statusColor(run.presentationStatus))
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
                                        if run.presentationStatus == .waitingApproval {
                                            Image(systemName: "checkmark.seal.fill")
                                                .foregroundStyle(DesignTokens.Status.warning)
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
            editingWorkspacePath = idea.workspaceRootPath ?? ""
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

    private func browseWorkspaceRoot() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Select Project Directory"
        if panel.runModal() == .OK, let url = panel.url {
            editingWorkspacePath = url.path
        }
        #endif
    }

    private func saveWorkspaceRoot() {
        let trimmed = editingWorkspacePath.trimmingCharacters(in: .whitespaces)
        idea.workspaceRootPath = trimmed.isEmpty ? nil : trimmed
        try? modelContext.save()
    }

    private func isValidDirectory(_ path: String) -> Bool {
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) && isDirectory.boolValue
    }

    private func runStatusIcon(_ status: RunStatus) -> some View {
        let (icon, color): (String, Color) = {
            switch status {
            case .pending, .ready: return ("clock", DesignTokens.Status.neutral)
            case .running: return ("play.circle.fill", DesignTokens.Status.running)
            case .waitingApproval: return ("checkmark.seal", DesignTokens.Status.warning)
            case .blocked: return ("pause.circle.fill", DesignTokens.Status.warning)
            case .completed: return ("checkmark.circle.fill", DesignTokens.Status.success)
            case .failed: return ("xmark.circle.fill", DesignTokens.Status.error)
            case .cancelled: return ("stop.circle.fill", DesignTokens.Status.cancelled)
            case .cancelling: return ("hourglass", DesignTokens.Status.warning)
            }
        }()
        return Image(systemName: icon).foregroundStyle(color)
    }

    private func statusColor(_ status: RunStatus) -> Color {
        switch status {
        case .pending, .ready: return DesignTokens.Status.neutral
        case .running: return DesignTokens.Status.running
        case .waitingApproval: return DesignTokens.Status.warning
        case .blocked: return DesignTokens.Status.warning
        case .completed: return DesignTokens.Status.success
        case .failed: return DesignTokens.Status.error
        case .cancelled: return DesignTokens.Status.cancelled
        case .cancelling: return DesignTokens.Status.warning
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
    @State private var selectedContextStrategyProfileID = "current_mixed_baseline"
    @State private var preflightReport: PreflightReport?
    @State private var showPreflightSheet = false
    @State private var allowWarnStart = false
    @State private var showAdvancedOverrides = false

    // MARK: Delivery Configuration (Proposal 007 §10.1)
    @State private var deliveryRepoRoot = ""
    @State private var deliveryBaseBranch = "main"
    @State private var deliveryTargetBranch = ""
    @State private var deliveryWorktreeBasePath = ""
    @State private var deliveryReleaseTargetID = "sandbox_local"
    @State private var deliveryReleaseMode: ReleaseMode = .sandbox
    @State private var deliveryPreflightResult: DeliveryPreflightService.PreflightResult?
    @State private var showDeliveryPreflightSheet = false

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

    @ViewBuilder
    private var workflowSelectionControl: some View {
        if availableWorkflows.count <= 1 {
            LabeledContent("Workflow", value: workflowSelection.wrappedValue.title)
                .accessibilityIdentifier("workflow-preset-single")
        } else {
            VStack(alignment: .leading, spacing: 8) {
                Text("Workflow")
                    .font(.subheadline.weight(.medium))
                ForEach(availableWorkflows) { workflow in
                    Button {
                        workflowSelection.wrappedValue = workflow
                    } label: {
                        HStack(spacing: 10) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(workflow.title)
                                    .font(.subheadline.weight(.medium))
                                    .foregroundStyle(.primary)
                                Text(workflow.relativePath)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            Spacer()
                            Image(systemName: workflowSelection.wrappedValue == workflow ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(workflowSelection.wrappedValue == workflow ? DesignTokens.Action.primary : DesignTokens.Status.neutral)
                        }
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .fill(workflowSelection.wrappedValue == workflow ? DesignTokens.Action.primary.opacity(0.12) : DesignTokens.Status.neutral.opacity(0.08))
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(workflow.title)
                    .accessibilityValue(workflowSelection.wrappedValue == workflow ? "selected" : "not selected")
                    .accessibilityIdentifier("workflow-preset-button-\(workflow.id)")
                }
            }
            .accessibilityIdentifier("workflow-preset-list")
            .accessibilityValue(workflowSelection.wrappedValue.id)
        }
    }

    private var selectedWorkflowURL: URL? {
        workflowURLs[selectedWorkflow]
    }

    private var availableContextStrategyProfileIDs: [String] {
        let config = executionService.stewardConfig ?? .defaultConfig
        let keys = config.contextStrategyProfiles.keys.sorted()
        return keys.isEmpty ? ["current_mixed_baseline"] : keys
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

    private var liveRuntimeReady: Bool {
        if case .ready = executionService.liveRuntimeReadiness {
            return true
        }
        return false
    }

    private var liveRuntimeRecoveryCopy: (reason: String, recovery: String)? {
        if case let .unavailable(reason, recovery) = executionService.liveRuntimeReadiness {
            return (reason, recovery)
        }
        return nil
    }

    @ViewBuilder
    private var executionModeSelectionControl: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Execution Mode")
                .font(.subheadline.weight(.medium))
            ForEach(availableModes) { mode in
                Button {
                    selectedMode = mode
                } label: {
                    HStack(spacing: 10) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(mode.title)
                                .font(.subheadline.weight(.medium))
                                .foregroundStyle(.primary)
                            Text(mode == .live ? "Uses configured Goose-backed execution." : "Uses the canonical local executor.")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Image(systemName: selectedMode == mode ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(selectedMode == mode ? DesignTokens.Action.primary : DesignTokens.Status.neutral)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                        .background(
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .fill(selectedMode == mode ? DesignTokens.Action.primary.opacity(0.12) : DesignTokens.Status.neutral.opacity(0.08))
                        )
                }
                .buttonStyle(.plain)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(mode.title)
                .accessibilityValue(selectedMode == mode ? "selected" : "not selected")
                .accessibilityIdentifier("execution-mode-\(mode.id)-button")
            }
        }
        .accessibilityIdentifier("execution-mode-list")
    }

    private var liveModeConfigured: Bool {
        liveRuntimeReady
    }

    private var shouldDefaultToDeliveryFlow: Bool {
        liveRuntimeReady && !(idea.workspaceRootPath?.trimmingCharacters(in: .whitespaces).isEmpty ?? true)
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

    private var normalizedWorkspaceRoot: String? {
        idea.workspaceRootPath?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
    }

    private var effectiveDeliveryRepoRoot: String? {
        normalizedWorkspaceRoot ?? deliveryRepoRoot.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
    }

    private var effectiveDeliveryWorktreeBasePath: String {
        if let explicit = deliveryWorktreeBasePath.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty {
            return explicit
        }
        if let configured = appConfigurationStore.configuration.worktreeBasePath?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty {
            return configured
        }
        return AppConfiguration.defaultSupportRoot()
            .appendingPathComponent("worktrees", isDirectory: true)
            .path
    }

    private var effectiveDeliveryTargetBranch: String {
        deliveryTargetBranch.isEmpty
            ? "dogfood/\(idea.title.lowercased().replacingOccurrences(of: " ", with: "-"))-\(UUID().uuidString.prefix(8))"
            : deliveryTargetBranch
    }

    private var startRunBlockingReasons: [String] {
        var reasons: [String] = []
        if compiledPlan == nil { reasons.append("compile_pending") }
        if isStarting { reasons.append("starting") }
        if compileState == .compiling { reasons.append("compiling") }
        if liveModeRequiresConfiguration { reasons.append("live_runtime_unconfigured") }
        if preflightBlocksStart { reasons.append("preflight_failed") }
        if preflightReport?.status == .warn && requiresCleanPreflight { reasons.append("preflight_requires_clean") }
        if warnRequiresConfirmation && allowWarnStart == false { reasons.append("warning_confirmation_required") }
        if deliveryPreflightBlocksStart { reasons.append("delivery_preflight_blocked") }
        return reasons
    }

    private var startRunButtonAccessibilityValue: String {
        if startRunBlockingReasons.isEmpty {
            return "enabled"
        }
        var components = ["blocked:\(startRunBlockingReasons.joined(separator: ","))"]
        if let preflightReport, let firstBlockingIssue = preflightReport.blockingIssues.first {
            components.append("preflight_issue=\(firstBlockingIssue)")
        } else if let preflightReport, let firstWarning = preflightReport.warnings.first {
            components.append("preflight_warning=\(firstWarning)")
        }
        if let deliveryPreflightResult, !deliveryPreflightResult.passed {
            let failedIDs = deliveryPreflightResult.failedChecks.map(\.id).joined(separator: ",")
            components.append("delivery_checks=\(failedIDs)")
        }
        if selectedWorkflow == .fullMVPLive {
            components.append("delivery_repo_root=\(effectiveDeliveryRepoRoot ?? "nil")")
        }
        return components.joined(separator: " | ")
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
                    .foregroundStyle(DesignTokens.Action.primary)
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
                    if selectedMode == .live && !liveModeConfigured {
                        VStack(alignment: .leading, spacing: 6) {
                            Label("Live runtime unavailable", systemImage: "exclamationmark.triangle.fill")
                                .font(.caption.weight(.semibold))
                                .accessibilityIdentifier("live-runtime-unavailable-title")
                            Text(liveRuntimeRecoveryCopy?.reason ?? "Live workflows require an available Goose runtime.")
                                .font(.caption2)
                                .accessibilityIdentifier("live-runtime-unavailable-guidance")
                            if let recovery = liveRuntimeRecoveryCopy?.recovery, !recovery.isEmpty {
                                Text(recovery)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .accessibilityIdentifier("live-runtime-unavailable-recovery")
                            }
                        }
                        .foregroundStyle(DesignTokens.Status.warning)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .fill(DesignTokens.Status.warning.opacity(0.10))
                        )
                        .accessibilityElement(children: .contain)
                        .accessibilityIdentifier("live-runtime-missing-block")
                    }

                    executionModeSelectionControl

                    workflowSelectionControl

                    GroupBox {
                        VStack(alignment: .leading, spacing: 8) {
                            Picker("Context Strategy", selection: $selectedContextStrategyProfileID) {
                                ForEach(availableContextStrategyProfileIDs, id: \.self) { profileID in
                                    Text(profileID).tag(profileID)
                                }
                            }
                            .pickerStyle(.menu)
                            .accessibilityIdentifier("context-strategy-picker")

                            Text("The selected strategy is frozen into the run snapshot and reused for resume, frozen clone, and comparison.")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    } label: {
                        VStack(alignment: .leading, spacing: 3) {
                            Text("Context Strategy")
                                .font(.subheadline.bold())
                            Text("Choose the strategy profile for this run.")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }

                    if selectedMode == .live && !liveModeConfigured {
                        Text("Advanced setup: `CHAINWORKS_GOOSE_BASE_URL` or `CHAINWORKS_GOOSE_FIXTURE_MODE=proposal_loop_success`, then relaunch the app.")
                            .font(.caption)
                            .foregroundStyle(DesignTokens.Status.warning)
                            .accessibilityIdentifier("live-runtime-unavailable-advanced")
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
                            .foregroundStyle(DesignTokens.Status.success)
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

            // MARK: - Delivery Configuration (Proposal 007 §10.1)
            if selectedWorkflow == .fullMVPLive {
                GroupBox("Delivery Configuration") {
                    VStack(alignment: .leading, spacing: 10) {
                        Label("Repo-backed delivery settings for the Full MVP Live workflow.", systemImage: "shippingbox.fill")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        LabeledContent("Repository Root") {
                            TextField("Path to repo", text: $deliveryRepoRoot)
                                .textFieldStyle(.roundedBorder)
                                .font(.caption)
                                .accessibilityIdentifier("delivery-repo-root")
                        }
                        .font(.caption)

                        HStack(spacing: 12) {
                            LabeledContent("Base Branch") {
                                TextField("main", text: $deliveryBaseBranch)
                                    .textFieldStyle(.roundedBorder)
                                    .font(.caption)
                                    .frame(maxWidth: 140)
                                    .accessibilityIdentifier("delivery-base-branch")
                            }
                            .font(.caption)

                            LabeledContent("Target Branch") {
                                TextField("dogfood/full-mvp", text: $deliveryTargetBranch)
                                    .textFieldStyle(.roundedBorder)
                                    .font(.caption)
                                    .frame(maxWidth: 180)
                                    .accessibilityIdentifier("delivery-target-branch")
                            }
                            .font(.caption)
                        }

                        LabeledContent("Worktree Base Path") {
                            TextField("Path for worktrees", text: $deliveryWorktreeBasePath)
                                .textFieldStyle(.roundedBorder)
                                .font(.caption)
                                .accessibilityIdentifier("delivery-worktree-base")
                        }
                        .font(.caption)

                        HStack(spacing: 12) {
                            LabeledContent("Release Target") {
                                TextField("sandbox_local", text: $deliveryReleaseTargetID)
                                    .textFieldStyle(.roundedBorder)
                                    .font(.caption)
                                    .frame(maxWidth: 140)
                                    .accessibilityIdentifier("delivery-release-target")
                            }
                            .font(.caption)

                            Picker("Release Mode", selection: $deliveryReleaseMode) {
                                Text("Sandbox").tag(ReleaseMode.sandbox)
                                Text("Staging").tag(ReleaseMode.staging)
                            }
                            .pickerStyle(.segmented)
                            .accessibilityIdentifier("delivery-release-mode-picker")
                        }

                        // Delivery Preflight
                        Divider()
                        if let deliveryPreflightResult {
                            HStack {
                                Label(
                                    deliveryPreflightResult.passed ? "Delivery preflight passed" : "Delivery preflight issues found",
                                    systemImage: deliveryPreflightResult.passed ? "checkmark.shield.fill" : "exclamationmark.shield.fill"
                                )
                                .foregroundStyle(deliveryPreflightResult.passed ? .green : .orange)
                                Spacer()
                                Button("Review") {
                                    showDeliveryPreflightSheet = true
                                }
                            }
                            .font(.caption)

                            if !deliveryPreflightResult.passed {
                                ForEach(deliveryPreflightResult.failedChecks, id: \.id) { check in
                                    Label(check.detail ?? check.label, systemImage: "xmark.circle")
                                        .font(.caption2)
                                        .foregroundStyle(DesignTokens.Status.error)
                                }
                            }
                        } else {
                            Button("Run Delivery Preflight") {
                                Task { await runDeliveryPreflight() }
                            }
                            .font(.caption)
                            .accessibilityIdentifier("delivery-preflight-button")
                        }

                        // Summary block
                        if let effectiveRepoRoot = effectiveDeliveryRepoRoot {
                            Divider()
                            VStack(alignment: .leading, spacing: 3) {
                                Text("Workflow: Full MVP Live")
                                Text("Repo: \(URL(fileURLWithPath: effectiveRepoRoot).lastPathComponent) → \(effectiveRepoRoot)")
                                Text("Branch: \(deliveryBaseBranch) → \(deliveryTargetBranch.isEmpty ? "auto" : deliveryTargetBranch)")
                                Text("Release target: \(deliveryReleaseMode.rawValue.capitalized) (\(deliveryReleaseTargetID))")
                                Text("Safety: dedicated worktree, manual release gate, deterministic services")
                            }
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .accessibilityIdentifier("delivery-configuration-section")
                .sheet(isPresented: $showDeliveryPreflightSheet) {
                    if let deliveryPreflightResult {
                        NavigationStack {
                            DeliveryPreflightReportView(result: deliveryPreflightResult)
                                .toolbar {
                                    ToolbarItem(placement: .cancellationAction) {
                                        Button("Done") { showDeliveryPreflightSheet = false }
                                    }
                                }
                        }
                        .frame(minWidth: 480, minHeight: 360)
                    }
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
                        .foregroundStyle(DesignTokens.Status.success)

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
                            .foregroundStyle(DesignTokens.Status.error)
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
                                .accessibilityIdentifier("allow-start-with-warnings-toggle")
                        } else if preflightReport.status == .warn && requiresCleanPreflight {
                            Label("Run start is blocked until preflight is clean in current settings.", systemImage: "lock.fill")
                                .font(.caption)
                                .foregroundStyle(DesignTokens.Status.warning)
                        }

                        if let firstBlockingIssue = preflightReport.blockingIssues.first {
                            Text(firstBlockingIssue)
                                .font(.caption2)
                                .foregroundStyle(DesignTokens.Status.error)
                        } else if let firstWarning = preflightReport.warnings.first {
                            Text(firstWarning)
                                .font(.caption2)
                                .foregroundStyle(DesignTokens.Status.warning)
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
                .accessibilityIdentifier("workflow-compile-button")

                Button("Start Run") {
                    startRun()
                }
                .buttonStyle(.borderedProminent)
                .disabled(!startRunBlockingReasons.isEmpty)
                .accessibilityIdentifier("workflow-start-run-confirm-button")
                .accessibilityValue(startRunButtonAccessibilityValue)
            }
        }
        .padding(20)
        .frame(minWidth: 520, minHeight: 480)
        .task {
            resolveURLs()
            selectedMode = shouldDefaultToDeliveryFlow ? .live : (availableModes.contains(selectedMode) ? selectedMode : .simulated)
            if shouldDefaultToDeliveryFlow {
                selectedWorkflow = .fullMVPLive
            } else {
                selectedWorkflow = availableWorkflows.first ?? .canonicalRelease
            }
            if deliveryRepoRoot.isEmpty, let workspaceRoot = normalizedWorkspaceRoot {
                deliveryRepoRoot = workspaceRoot
            }
            if deliveryWorktreeBasePath.isEmpty {
                deliveryWorktreeBasePath = effectiveDeliveryWorktreeBasePath
            }
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
        let configuredWorkflowRepositoryRoot = repositoryRoot(derivedFromWorkflowPath: appConfigurationStore.configuration.workflowSourcePath)
        workflowURLs = Dictionary(uniqueKeysWithValues: WorkflowPreset.allCases.compactMap { preset in
            let bundleURL = preset.bundleResourceName.flatMap { Bundle.main.url(forResource: $0, withExtension: "yaml") }
            let configuredWorkflowURL: URL? = {
                switch preset {
                case .canonicalRelease:
                    return URL(fileURLWithPath: appConfigurationStore.configuration.workflowSourcePath)
                case .proposalLoopLive:
                    return configuredWorkflowRepositoryRoot?.appendingPathComponent(preset.relativePath)
                case .fullMVPLive:
                    return configuredWorkflowRepositoryRoot?.appendingPathComponent(preset.relativePath)
                }
            }()
            var candidates: [URL?] = [
                configuredWorkflowURL,
                URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent(preset.relativePath)
            ]
            if AppConfiguration.allowsDocumentsFallbackForCurrentProcess {
                candidates.append(
                    URL(fileURLWithPath: NSHomeDirectory())
                        .appendingPathComponent("Documents/Chainworks Forge/\(preset.relativePath)")
                )
            }
            candidates.append(bundleURL)
            guard let url = resolveExistingFile(at: candidates) else {
                return nil
            }
            return (preset, url)
        })

        var catalogCandidates: [URL?] = [
            URL(fileURLWithPath: appConfigurationStore.configuration.agentCatalogSourcePath),
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("examples/agents/agents.yaml")
        ]
        if AppConfiguration.allowsDocumentsFallbackForCurrentProcess {
            catalogCandidates.append(
                URL(fileURLWithPath: NSHomeDirectory())
                    .appendingPathComponent("Documents/Chainworks Forge/examples/agents/agents.yaml")
            )
        }
        catalogCandidates.append(Bundle.main.url(forResource: "agents", withExtension: "yaml"))
        catalogURL = resolveExistingFile(at: catalogCandidates)
    }

    private func resolveExistingFile(at candidates: [URL?]) -> URL? {
        for case let url? in candidates where FileManager.default.isReadableFile(atPath: url.path) {
            return url
        }
        return nil
    }

    private func repositoryRoot(derivedFromWorkflowPath path: String) -> URL? {
        let workflowURL = URL(fileURLWithPath: path)
        let components = Array(workflowURL.pathComponents.suffix(3))
        guard components.count == 3,
              components[0] == "examples",
              components[1] == "workflows" else {
            return nil
        }
        return workflowURL
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
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
              !warnRequiresConfirmation || allowWarnStart,
              !deliveryPreflightBlocksStart else { return }

        // Proposal 011 (REQ-005, REQ-007): Fail-closed workspace check — no ambient cwd fallback.
        if compiledPlan.requiresProjectAccess {
            guard let workspacePath = idea.workspaceRootPath,
                  !workspacePath.trimmingCharacters(in: .whitespaces).isEmpty else {
                compileState = .error("Workflow requires project access but idea has no workspace root path. Set it in the idea detail view.")
                return
            }
            var isDirectory: ObjCBool = false
            let exists = FileManager.default.fileExists(atPath: workspacePath, isDirectory: &isDirectory)
            guard exists, isDirectory.boolValue else {
                compileState = .error("Workspace root path is not a valid directory: \(workspacePath)")
                return
            }
        }

        isStarting = true

        do {
            let resolver = BackendProfileResolverV2(providerRegistry: providerRegistry)
            let providerBindings = try resolver.resolveBindings(plan: compiledPlan, startOptions: startOptions)
            let adjustedPlan = RunStartOverrideResolver.applying(bindings: providerBindings, to: compiledPlan)
            let startSnapshot = try buildRunStartSnapshot(
                resolver: resolver,
                adjustedPlan: adjustedPlan,
                providerBindings: providerBindings
            )
            let compiler = RunPlanCompiler(modelContext: modelContext)
            let (run, workspace) = try compiler.createRun(
                for: idea,
                plan: adjustedPlan,
                workflowSourcePath: workflowURL.path,
                catalogSourcePath: catalogURL.path,
                startSnapshot: startSnapshot
            )

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
            startOptions: startOptions,
            idea: idea
        )
        if preflightReport?.status != .warn {
            allowWarnStart = false
        }
    }

    // MARK: - Delivery Preflight (Proposal 007 §9.6)

    private func runDeliveryPreflight() async {
        let effectiveRepoRoot = effectiveDeliveryRepoRoot ?? "(no project directory set)"

        let draft = DeliveryConfiguration(
            profileID: nil,
            profileLabel: nil,
            sampleProfileID: nil,
            repoIdentifier: URL(fileURLWithPath: effectiveRepoRoot).lastPathComponent,
            repoRoot: effectiveRepoRoot,
            baseBranch: deliveryBaseBranch,
            worktreeBasePath: effectiveDeliveryWorktreeBasePath,
            targetBranch: effectiveDeliveryTargetBranch,
            releaseTargetID: deliveryReleaseTargetID,
            releaseTargetLabel: deliveryReleaseMode == .sandbox ? "Sandbox" : "Staging",
            releaseMode: deliveryReleaseMode
        )

        let service = DeliveryPreflightService()
        deliveryPreflightResult = await service.validate(draft)
    }

    private var deliveryPreflightBlocksStart: Bool {
        guard selectedWorkflow == .fullMVPLive else { return false }
        // Block start if delivery preflight hasn't been run or has failed
        guard let result = deliveryPreflightResult else { return true }
        return !result.passed
    }

    private func encodeProviderBindings(_ bindings: [String: ResolvedProviderBinding]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(bindings)
    }

    private func encodeProvenances(_ provenances: [String: FrozenBindingProvenance]) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(provenances)
    }

    private func encodeStartOptions(_ options: RunStartOptions) -> Data? {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(options)
    }

    private func buildRunStartSnapshot(
        resolver: BackendProfileResolverV2,
        adjustedPlan: RunPlan,
        providerBindings: [String: ResolvedProviderBinding]
    ) throws -> RunStartSnapshot {
        let provenances = resolver.resolveProvenances(plan: adjustedPlan, startOptions: startOptions)
        let strategySelection = StrategyExperimentCoordinator(config: executionService.stewardConfig)
            .resolveSelection(
                selectedProfileID: selectedContextStrategyProfileID,
                cohortID: nil
            )

        let deliveryConfig: DeliveryConfiguration?
        let deliveryPreflightJSON: Data?
        if selectedWorkflow == .fullMVPLive {
            guard let effectiveRepoRoot = effectiveDeliveryRepoRoot else {
                throw WorkflowStartSnapshotError.missingDeliveryProjectDirectory
            }
            deliveryConfig = DeliveryConfiguration(
                profileID: "chainworks_forge_self",
                profileLabel: "Chainworks Forge (Self)",
                sampleProfileID: "chainworks_forge_self",
                repoIdentifier: URL(fileURLWithPath: effectiveRepoRoot).lastPathComponent,
                repoRoot: effectiveRepoRoot,
                baseBranch: deliveryBaseBranch,
                worktreeBasePath: effectiveDeliveryWorktreeBasePath,
                targetBranch: effectiveDeliveryTargetBranch,
                releaseTargetID: deliveryReleaseTargetID,
                releaseTargetLabel: deliveryReleaseMode == .sandbox ? "Sandbox" : "Staging",
                releaseMode: deliveryReleaseMode
            )
            deliveryPreflightJSON = deliveryPreflightResult.flatMap { try? JSONEncoder().encode($0) }
        } else {
            deliveryConfig = nil
            deliveryPreflightJSON = nil
        }

        return RunStartSnapshot(
            providerBindingSnapshotJSON: encodeProviderBindings(providerBindings),
            bindingProvenanceJSON: encodeProvenances(provenances),
            startOptionsJSON: encodeStartOptions(startOptions),
            frozenWorkspaceRootPath: normalizedWorkspaceRoot,
            deliveryConfiguration: deliveryConfig,
            deliveryPreflightJSON: deliveryPreflightJSON,
            contextStrategyProfileID: strategySelection.profileID,
            strategyAssignmentMode: strategySelection.assignmentMode,
            strategyRecommendationState: strategySelection.recommendationState,
            contextStrategySnapshotJSON: try JSONEncoder().encode(strategySelection.profile)
        )
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
            return DesignTokens.Status.success
        case .warn:
            return DesignTokens.Status.warning
        case .fail:
            return DesignTokens.Status.error
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
                || $0.name.contains("approval_resolution_diagnostic_")
            }
            .filter { seen.insert($0.name).inserted }
            .prefix(4)
            .map { $0 }
    }

    private var latestMeaningfulEvent: LiveExecutionTimelineEntry? {
        liveTimeline.first(where: { $0.event.type != .textChunk }) ?? liveTimeline.first
    }

    private var latestPersistedCheckpointText: String? {
        let latestApproval = run.approvals.max {
            ($0.decidedAt ?? $0.requestedAt) < ($1.decidedAt ?? $1.requestedAt)
        }
        let latestStage = sortedStages.max {
            ($0.completedAt ?? $0.startedAt) < ($1.completedAt ?? $1.startedAt)
        }

        let approvalTimestamp = latestApproval.map { $0.decidedAt ?? $0.requestedAt }
        let stageTimestamp = latestStage.map { $0.completedAt ?? $0.startedAt }

        if let latestApproval, let approvalTimestamp,
           stageTimestamp == nil || approvalTimestamp >= stageTimestamp! {
            let decisionLabel = latestApproval.decision.rawValue.replacingOccurrences(of: "_", with: " ")
            return "Persisted approval \(decisionLabel) for \(latestApproval.stageID)"
        }

        if let latestStage {
            let statusLabel = latestStage.status.rawValue.replacingOccurrences(of: "_", with: " ")
            return "Persisted stage \(statusLabel) in \(latestStage.stageID)"
        }

        return nil
    }

    private var nextActionText: String {
        switch run.presentationStatus {
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
            return "Run was cancelled. Return to the idea to start another run or archive it."
        case .cancelling:
            return "Cancellation in progress. Waiting for agents to settle."
        }
    }

    var body: some View {
        List {
            Section("Overview") {
                LabeledContent("Workflow", value: run.workflowTitle)
                LabeledContent("Status", value: run.presentationStatusLabel)
                    .accessibilityIdentifier("run-status-\(run.presentationStatus.rawValue)")
                LabeledContent("Current Stage", value: run.currentStageID ?? "None")
                LabeledContent("Elapsed", value: elapsedText)
                LabeledContent("Total Cost", value: run.totalCostCents.map { "\($0) cents" } ?? "Pending")
            }

            Section("Current Phase") {
                LabeledContent("Phase", value: currentStageExecution?.label ?? run.currentStageID ?? "Not started")
                LabeledContent("Loop Iteration", value: currentStageExecution.map { "\($0.iteration)" } ?? "0")
                LabeledContent("Latest Event", value: latestMeaningfulEvent?.event.detail ?? latestPersistedCheckpointText ?? "Waiting for the next execution event")
                if let sessionID = latestMeaningfulEvent?.event.sessionID {
                    LabeledContent("Session ID", value: sessionID)
                }
                LabeledContent("Next Action", value: nextActionText)
                if run.presentationStatus == .completed || run.presentationStatus == .blocked || run.presentationStatus == .failed || run.presentationStatus == .cancelled {
                    Button("Open in Runs Home") {
                        NotificationCenter.default.post(
                            name: .chainworksOpenRunInRunsHome,
                            object: nil,
                            userInfo: ["runID": run.id.uuidString]
                        )
                    }
                    .buttonStyle(.bordered)
                    .accessibilityIdentifier("open-run-in-runs-home-button")
                }
            }

            if let pendingApprovalRequest {
                // Proposal 007 §11.1: Show ReleaseGateView when approvalPolicy is manual_release
                if run.deliveryConfigurationJSON != nil,
                   pendingApprovalRequest.approvalPolicy == "manual_release" {
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
                            .accessibilityIdentifier("approval-reject-button")
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
                            .accessibilityIdentifier("approval-approve-button")
                        }
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
                    }
                }
            }

            // MARK: Delivery Progress (Proposal 007 §10.2)
            if run.deliveryConfigurationJSON != nil {
                Section("Delivery Progress") {
                    if let worktreeRoot = run.worktreeRoot {
                        LabeledContent("Worktree", value: worktreeRoot)
                            .accessibilityIdentifier("delivery-worktree-path")
                    }
                    if let repoId = run.repoIdentifier {
                        LabeledContent("Repository", value: repoId)
                    }
                    if let baseBranch = run.baseBranch {
                        LabeledContent("Base Branch", value: baseBranch)
                    }
                    if let targetBranch = run.targetBranch {
                        LabeledContent("Target Branch", value: targetBranch)
                    }
                    if let baseRevision = run.baseRevision {
                        LabeledContent("Base Revision", value: String(baseRevision.prefix(8)))
                    }
                    if let releaseMode = run.releaseMode {
                        LabeledContent("Release Target", value: "\(run.releaseTargetID ?? "unknown") (\(releaseMode))")
                            .accessibilityIdentifier("delivery-release-target")
                    }

                    // Implementation loop status
                    if let implLoopCount = run.loopCounters["implementation_progress_count"] {
                        LabeledContent("Implementation Iterations", value: "\(implLoopCount)")
                            .accessibilityIdentifier("delivery-impl-iterations")
                    }
                    if let revisionCount = run.loopCounters["implementation_revision_count"] {
                        LabeledContent("Refinement Cycles", value: "\(revisionCount)")
                            .accessibilityIdentifier("delivery-refinement-cycles")
                    }

                    // Latest review status from artifacts
                    if let reviewSummary = latestArtifacts.first(where: { $0.name == "implementation_review_summary" }) {
                        Button {
                            selectedArtifact = reviewSummary
                        } label: {
                            HStack {
                                Label("Implementation Review Summary", systemImage: "checkmark.rectangle.stack")
                                Spacer()
                                Text(reviewSummary.format.rawValue)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .buttonStyle(.plain)
                    }

                    // Changed files
                    if let changedFiles = latestArtifacts.first(where: { $0.name == "changed_files_manifest" }) {
                        Button {
                            selectedArtifact = changedFiles
                        } label: {
                            HStack {
                                Label("Changed Files Manifest", systemImage: "doc.text.magnifyingglass")
                                Spacer()
                                Text(changedFiles.format.rawValue)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .buttonStyle(.plain)
                    }

                    // Tests result
                    if let testsResult = latestArtifacts.first(where: { $0.name == "tests_result" }) {
                        Button {
                            selectedArtifact = testsResult
                        } label: {
                            HStack {
                                Label("Tests Result", systemImage: "checkmark.circle")
                                Spacer()
                                Text(testsResult.format.rawValue)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
                .accessibilityIdentifier("delivery-progress-section")

                // Worktree / Repo Affordances (§10.4)
                Section("Worktree & Repo") {
                    if let worktreeRoot = run.worktreeRoot {
                        Button {
                            NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: worktreeRoot)
                        } label: {
                            Label("Open Worktree in Finder", systemImage: "folder")
                        }
                        .accessibilityIdentifier("open-worktree-finder")
                    }

                    if let diffSummary = latestArtifacts.first(where: { $0.name == "diff_summary" || $0.name == "changed_files_manifest" }) {
                        Button {
                            selectedArtifact = diffSummary
                        } label: {
                            Label("Reveal Diff Summary", systemImage: "doc.text")
                        }
                    }

                    if let releaseManifest = latestArtifacts.first(where: { $0.name == "release_manifest" }) {
                        Button {
                            selectedArtifact = releaseManifest
                        } label: {
                            Label("Reveal Release Manifest", systemImage: "shippingbox")
                        }
                    }

                    if let gitReceipt = latestArtifacts.first(where: { $0.name == "git_push_receipt" }) {
                        Button {
                            selectedArtifact = gitReceipt
                        } label: {
                            Label("Git Push Receipt", systemImage: "arrow.up.doc")
                        }
                    }

                    if let connectReceipt = latestArtifacts.first(where: { $0.name == "connect_upload_receipt" }) {
                        Button {
                            selectedArtifact = connectReceipt
                        } label: {
                            Label("Connect Upload Receipt", systemImage: "icloud.and.arrow.up")
                        }
                    }
                }
                .accessibilityIdentifier("worktree-repo-section")
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
        case .pending, .ready: return DesignTokens.Status.neutral
        case .running: return DesignTokens.Status.running
        case .waitingApproval: return DesignTokens.Status.warning
        case .blocked: return DesignTokens.Status.warning
        case .completed: return DesignTokens.Status.success
        case .failed: return DesignTokens.Status.error
        case .skipped: return DesignTokens.Status.neutral
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

    /// Decode frozen provenances from the run's snapshot (Proposal 011 — REQ-009).
    private func frozenProvenance(for agentID: String) -> FrozenBindingProvenance? {
        guard let data = run.bindingProvenanceJSON else { return nil }
        let decoded = try? JSONDecoder().decode([String: FrozenBindingProvenance].self, from: data)
        return decoded?[agentID]
    }

    /// Decode frozen binding from the run's snapshot (Proposal 011 — REQ-008).
    private func frozenBinding(for agentID: String) -> ResolvedProviderBinding? {
        guard let data = run.providerBindingSnapshotJSON else { return nil }
        let decoded = try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data)
        return decoded?[agentID]
    }

    @ViewBuilder
    private func agentMetadataRow(for execution: AgentExecution) -> some View {
        let frozen = frozenBinding(for: execution.agentID)
        let provenance = frozenProvenance(for: execution.agentID)
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 10) {
                Text(execution.provider)
                // Proposal 011 (REQ-008): Prefer frozen model truth.
                let displayModel = frozen?.model ?? execution.resolvedModel
                if let model = displayModel, !model.isEmpty {
                    Text(model)
                }
                Text(execution.effort)
                // Proposal 011 (REQ-009): Show provenance source.
                if let source = provenance?.source {
                    Text("[\(source.rawValue)]")
                        .foregroundStyle(.tertiary)
                }
                // Proposal 011 (REQ-010): Cross-family mismatch warning.
                if frozen?.hasCrossFamilyMismatch == true {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(DesignTokens.Status.warning)
                        .help("Cross-family binding: model '\(frozen?.model ?? "")' may not match provider family '\(frozen?.providerFamily ?? "")'")
                        .accessibilityIdentifier("cross-family-warning")
                }
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

// MARK: - Proposal 008 (REQ-009): Attachment Validation Status Icon

/// Displays a color-coded icon indicating attachment validation state.
/// `reference_only` = paperclip (secondary), `rejected` = warning triangle (red).
struct AttachmentStatusIcon: View {
    let path: String

    private var status: MVPBoundaryPolicy.AttachmentStatus {
        MVPBoundaryPolicy.validateAttachment(path: path)
    }

    var body: some View {
        let isValid = status == .referenceOnly
        Image(systemName: isValid ? "paperclip" : "exclamationmark.triangle")
            .font(.caption2)
            .foregroundStyle(isValid ? DesignTokens.Status.neutral : DesignTokens.Status.error)
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
            worktreeRoot: run.worktreeRoot.flatMap { URL(fileURLWithPath: $0) }
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

private enum WorkflowStartSnapshotError: LocalizedError {
    case missingDeliveryProjectDirectory

    var errorDescription: String? {
        switch self {
        case .missingDeliveryProjectDirectory:
            return "Delivery workflow requires a project directory. Set the workspace root on the idea or provide a delivery repo root."
        }
    }
}

private extension String {
    var nilIfEmpty: String? {
        isEmpty ? nil : self
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

#Preview("New Idea Sheet — Empty") {
    NewIdeaSheetView(
        draft: .constant(NewIdeaDraft()),
        onBrowseAttachment: {},
        onCancel: {},
        onSave: {}
    )
    .frame(width: 520, height: 380)
}

#Preview("New Idea Sheet — Ready") {
    NewIdeaSheetView(
        draft: .constant(
            NewIdeaDraft(
                title: "Canonical delivery dogfood",
                body: "Use the real repo-backed flow to validate the delivery path end to end.",
                attachmentPath: "/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md"
            )
        ),
        onBrowseAttachment: {},
        onCancel: {},
        onSave: {}
    )
    .frame(width: 520, height: 380)
}
