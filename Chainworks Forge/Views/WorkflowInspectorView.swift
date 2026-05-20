import SwiftUI

struct WorkflowInspectorView: View {
    @State private var fullState: LoadState<WorkflowDefinition> = .loading
    @State private var compactState: LoadState<CompactWorkflowDefinition> = .loading
    @State private var selectedView: WorkflowViewMode = .full
    
    enum SortMode: String, CaseIterable {
        case execution = "Execution Order"
        case source = "Source Order"
    }
    @State private var sortMode: SortMode = .execution
    
    @State private var catalogForValidation: AgentCatalog?
    @State private var overrideWorkflowURL: URL?
    @State private var overrideCompactURL: URL?

    let workflowURL: URL?
    let compactWorkflowURL: URL?
    let catalogURL: URL?

    private var effectiveWorkflowURL: URL? { overrideWorkflowURL ?? workflowURL }
    private var effectiveCompactURL: URL? { overrideCompactURL ?? compactWorkflowURL }

    enum WorkflowViewMode: String, CaseIterable {
        case full = "Full Workflow"
        case compact = "Compact Preview"
    }

    var body: some View {
        VStack(spacing: 0) {
            Picker("View", selection: $selectedView) {
                ForEach(WorkflowViewMode.allCases, id: \.self) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)
            .padding(.vertical, 8)

            switch selectedView {
            case .full:
                fullWorkflowView
            case .compact:
                compactWorkflowView
            }
        }
        .task { loadAll() }
    }

    // MARK: - Full Workflow

    @ViewBuilder
    private var fullWorkflowView: some View {
        switch fullState {
        case .loading:
            VStack(alignment: .leading, spacing: 14) {
                ForEach(0..<10) { _ in
                    VStack(alignment: .leading, spacing: 6) {
                        ForgeSkeleton.headline(width: 180)
                        ForgeSkeleton.text(width: 120)
                    }
                    .padding(.vertical, 4)
                }
            }
            .padding()
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        case .loaded(let workflow, let issues):
            fullWorkflowContent(workflow, issues: issues)
        case .fileNotFound(let path):
            fileNotFoundView(path: path) { openWorkflowFilePicker() }
        case .decodeError(let path, let error):
            decodeErrorView(path: path, error: error) { loadFull() }
        }
    }

    private func fullWorkflowContent(_ workflow: WorkflowDefinition, issues: [ValidationIssue]) -> some View {
        let errorCount = issues.filter { $0.severity == .error }.count
        let warnCount = issues.filter { $0.severity == .warning }.count
        let sorted = sortedStates

        return VStack(spacing: 0) {
            fullSummaryStrip(workflow: workflow, errorCount: errorCount, warnCount: warnCount)
            
            Picker("Sort Mode", selection: $sortMode) {
                ForEach(SortMode.allCases, id: \.self) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)
            .padding(.vertical, 8)

            NavigationSplitView {
                List {
                    if sortMode == .execution {
                        Section("Primary Path") {
                            ForEach(sorted.primaryPath, id: \.self) { stateID in
                                NavigationLink {
                                    stateDetail(stateID: stateID, state: workflow.states[stateID]!)
                                } label: {
                                    stateRow(stateID: stateID, state: workflow.states[stateID]!)
                                }
                            }
                        }
                        
                        if !sorted.branches.isEmpty {
                            Section("Branches") {
                                ForEach(sorted.branches, id: \.self) { stateID in
                                    NavigationLink {
                                        stateDetail(stateID: stateID, state: workflow.states[stateID]!)
                                    } label: {
                                        stateRow(stateID: stateID, state: workflow.states[stateID]!)
                                    }
                                }
                            }
                        }
                    } else {
                        Section("All States (Source Order)") {
                            ForEach(sorted.primaryPath, id: \.self) { stateID in
                                NavigationLink {
                                    stateDetail(stateID: stateID, state: workflow.states[stateID]!)
                                } label: {
                                    stateRow(stateID: stateID, state: workflow.states[stateID]!)
                                }
                            }
                        }
                    }
                    
                    if !sorted.unreachable.isEmpty {
                        Section("Unreachable States") {
                            ForEach(sorted.unreachable, id: \.self) { stateID in
                                NavigationLink {
                                    stateDetail(stateID: stateID, state: workflow.states[stateID]!)
                                } label: {
                                    stateRow(stateID: stateID, state: workflow.states[stateID]!)
                                }
                            }
                        }
                    }
                }
                .accessibilityIdentifier("workflow-state-list")
            } detail: {
                if !issues.isEmpty {
                    issuesView(issues)
                } else {
                    Text("Select a state")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    struct SortedStates {
        let primaryPath: [String]
        let branches: [String]
        let unreachable: [String]
    }

    private var sortedStates: SortedStates {
        guard case .loaded(let workflow, _) = fullState else { 
            return SortedStates(primaryPath: [], branches: [], unreachable: []) 
        }
        
        var primary: [String] = []
        var branches: [String] = []
        var visited = Set<String>()
        
        // DFS to find execution path; skip unknown transition targets to prevent force-unwrap crash.
        func traverse(_ id: String, isPrimary: Bool) {
            guard !visited.contains(id), workflow.states[id] != nil else { return }
            visited.insert(id)
            
            if isPrimary {
                primary.append(id)
            } else {
                branches.append(id)
            }
            
            if let transitions = workflow.states[id]?.transitions {
                for (index, t) in transitions.enumerated() {
                    traverse(t.to, isPrimary: isPrimary && index == 0)
                }
            }
        }
        
        traverse(workflow.initialState, isPrimary: true)
        
        // Unvisited states (islands)
        let unvisited = workflow.states.keys.sorted().filter { !visited.contains($0) }
        
        if sortMode == .source {
            // P036: Use explicit state_order from YAML only when reconcilable with the known
            // state graph (no phantom IDs, initialState present). Fall back to execution-biased
            // deterministic order for hostile orderings (L1 fix).
            if let order = workflow.stateOrder {
                let knownIDs = Set(workflow.states.keys)
                let hasPhantoms = order.contains { !knownIDs.contains($0) }
                let missingInitial = !order.contains(workflow.initialState)
                if !hasPhantoms && !missingInitial {
                    return SortedStates(primaryPath: order, branches: [], unreachable: [])
                }
            }

            // Fallback: Combine primary, branches, and unvisited into a single deterministic list
            let combined = primary + branches + unvisited
            return SortedStates(primaryPath: combined, branches: [], unreachable: [])
        }
        
        return SortedStates(primaryPath: primary, branches: branches, unreachable: unvisited)
    }

    private func fullSummaryStrip(workflow: WorkflowDefinition, errorCount: Int, warnCount: Int) -> some View {
        let gateCount = workflow.states.values.filter { $0.approval == "required" }.count
        let loopCount = workflow.states.values.filter { $0.loop != nil }.count
        let color: Color = errorCount > 0 ? .red : (warnCount > 0 ? .yellow : .green)

        return HStack {
            Image(systemName: errorCount > 0 ? "xmark.circle.fill" : "checkmark.circle.fill")
                .foregroundStyle(color)
            Text("\(workflow.states.count) states \u{00B7} \(gateCount) gates \u{00B7} \(loopCount) loops")
                .accessibilityIdentifier("workflow-state-count")
            Spacer()
            if errorCount > 0 || warnCount > 0 {
                Text("\(errorCount) errors, \(warnCount) warnings")
                    .foregroundStyle(color)
            }
        }
        .accessibilityElement(children: .contain)
        .font(.caption)
        .padding(.horizontal)
        .padding(.vertical, 6)
        .background(color.opacity(0.1))
    }

    private func stateRow(stateID: String, state: WorkflowState) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                if state.type == "manual_gate" { Text("\u{270B}") }
                else if state.type == "start" { Text("\u{25B6}\u{FE0F}") }
                else if state.type == "end" { Text("\u{1F3C1}") }
                Text(state.label)
                    .font(.headline)
                    .lineLimit(1)
                
                if let transitions = state.transitions, transitions.count > 1 {
                    Image(systemName: "arrow.branch")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .help("Ambiguous transitions")
                }
            }
            Text(stateID)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func stateDetail(stateID: String, state: WorkflowState) -> some View {
        Form {
            Section("State") {
                LabeledContent("ID", value: stateID)
                LabeledContent("Label", value: state.label)
                if let type = state.type {
                    LabeledContent("Type", value: type)
                }
                LabeledContent("Owner", value: state.owner)
                if state.approval == "required" {
                    LabeledContent("Approval", value: "required")
                }
            }
            if let loop = state.loop {
                Section("Loop") {
                    LabeledContent("Counter", value: loop.counter)
                    LabeledContent("Max", value: loop.max)
                }
            }
            if let run = state.run {
                Section("Run Block") {
                    runBlockContent(run)
                }
            }
            if let runAfter = state.runAfterApproval {
                Section("Run After Approval") {
                    runBlockContent(runAfter)
                }
            }
            if let transitions = state.transitions, !transitions.isEmpty {
                Section("Transitions") {
                    ForEach(Array(transitions.enumerated()), id: \.offset) { _, t in
                        VStack(alignment: .leading) {
                            Text("\u{2192} \(t.to)").font(.headline)
                            Text("when: \(t.when)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(state.label)
    }

    private func runBlockContent(_ block: RunBlock) -> some View {
        Group {
            if let seq = block.sequence {
                ForEach(Array(seq.enumerated()), id: \.offset) { _, task in
                    LabeledContent("sequence: \(task.agent)", value: task.task)
                }
            }
            if let par = block.parallel {
                ForEach(Array(par.enumerated()), id: \.offset) { _, task in
                    LabeledContent("parallel: \(task.agent)", value: task.task)
                }
            }
            if let then = block.then {
                ForEach(Array(then.enumerated()), id: \.offset) { _, task in
                    LabeledContent("then: \(task.agent)", value: task.task)
                }
            }
        }
    }

    // MARK: - Compact Workflow

    @ViewBuilder
    private var compactWorkflowView: some View {
        switch compactState {
        case .loading:
            VStack(alignment: .leading, spacing: 12) {
                ForgeSkeleton.headline(width: 200)
                ForgeSkeleton.text(width: nil)
                ForgeSkeleton.text(width: nil)
                ForgeSkeleton.text(width: 250)
            }
            .padding()
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        case .loaded(let compact, let issues):
            compactContent(compact, issues: issues)
        case .fileNotFound(let path):
            fileNotFoundView(path: path) { openCompactFilePicker() }
        case .decodeError(let path, let error):
            decodeErrorView(path: path, error: error) { loadCompact() }
        }
    }

    private func compactContent(_ compact: CompactWorkflowDefinition, issues: [ValidationIssue]) -> some View {
        let errorCount = issues.filter { $0.severity == .error }.count
        let warnCount = issues.filter { $0.severity == .warning }.count
        let color: Color = errorCount > 0 ? .red : (warnCount > 0 ? .yellow : .green)

        return VStack(spacing: 0) {
            HStack {
                Image(systemName: "eye").foregroundStyle(.orange)
                Text("Compact format \u{2014} preview only, not executable")
                    .font(.caption).foregroundStyle(.orange)
                Spacer()
                if errorCount > 0 || warnCount > 0 {
                    Text("\(errorCount) errors, \(warnCount) warnings")
                        .font(.caption).foregroundStyle(color)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 6)
            .background(Color.orange.opacity(0.08))

            List(compact.workflow.stages) { stage in
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(stage.id).font(.headline)
                        Spacer()
                        Text(stage.type)
                            .font(.caption)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(typeColor(stage.type).opacity(0.15))
                            .clipShape(Capsule())
                    }
                    if let agent = stage.agent {
                        Text("agent: \(agent)").font(.caption).foregroundStyle(.secondary)
                    }
                    if let agents = stage.agents {
                        Text("agents: \(agents.joined(separator: ", "))").font(.caption).foregroundStyle(.secondary)
                    }
                    if let needs = stage.needs {
                        Text("needs: \(needs.joined(separator: ", "))").font(.caption2).foregroundStyle(.tertiary)
                    }
                    // Display gate (proposal contract)
                    if let gate = stage.gate {
                        Text("gate: \(gate.require.joined(separator: ", "))")
                            .font(.caption2)
                            .foregroundStyle(.orange)
                    }
                }
            }

            if !issues.isEmpty {
                issuesView(issues)
                    .frame(maxHeight: 200)
            }
        }
    }

    private func typeColor(_ type: String) -> Color {
        switch type {
        case "approval": return .orange
        case "fanout": return .purple
        case "single": return .blue
        default: return .gray
        }
    }

    // MARK: - Shared Error Views

    private func fileNotFoundView(path: String, openFile: @escaping () -> Void) -> some View {
        ForgeEmptyState(
            title: "File Not Found",
            systemImage: "doc.questionmark",
            description: path,
            actionTitle: "Open File\u{2026}",
            action: openFile
        )
    }

    private func decodeErrorView(path: String, error: Error, reload: @escaping () -> Void) -> some View {
        ForgeEmptyState(
            title: "Decode Error",
            systemImage: "exclamationmark.triangle",
            description: "\(path)\n\n\(error.localizedDescription)",
            actionTitle: "Retry",
            action: reload
        )
    }

    private func issuesView(_ issues: [ValidationIssue]) -> some View {
        List(issues) { issue in
            HStack(alignment: .top) {
                Image(systemName: issue.severity == .error ? "xmark.circle.fill" : "exclamationmark.triangle.fill")
                    .foregroundStyle(issue.severity == .error ? .red : .yellow)
                VStack(alignment: .leading) {
                    Text(issue.message).font(.caption)
                    if let loc = issue.location {
                        Text(loc).font(.caption2).foregroundStyle(.tertiary)
                    }
                }
            }
        }
    }

    // MARK: - Loading

    private func loadAll() {
        loadCatalog()
        loadFull()
        loadCompact()
    }

    private func loadFull() {
        guard let url = effectiveWorkflowURL else {
            fullState = .fileNotFound("No workflow URL configured")
            return
        }
        do {
            let workflow = try YAMLParser.loadWorkflow(from: url)
            // Use full validateAll when catalog is available (proposal contract)
            let issues: [ValidationIssue]
            if let catalog = catalogForValidation {
                issues = YAMLValidator.validateAll(workflow: workflow, catalog: catalog)
            } else {
                issues = YAMLValidator.validateStateGraph(workflow)
                    + YAMLValidator.validateRunBlockSemantics(workflow)
            }
            fullState = .loaded(workflow, issues)
        } catch let error as YAMLParserError {
            switch error {
            case .fileNotFound(let path): fullState = .fileNotFound(path)
            case .fileReadFailed(let path, let inner): fullState = .decodeError(path, inner)
            case .decodingFailed(let path, let inner): fullState = .decodeError(path, inner)
            }
        } catch {
            fullState = .decodeError(url.path, error)
        }
    }

    private func loadCompact() {
        guard let url = effectiveCompactURL else {
            compactState = .fileNotFound("No compact workflow URL configured")
            return
        }
        do {
            let compact = try YAMLParser.loadCompactWorkflow(from: url)
            let issues = CompactWorkflowValidator.validate(compact)
            compactState = .loaded(compact, issues)
        } catch let error as YAMLParserError {
            switch error {
            case .fileNotFound(let path): compactState = .fileNotFound(path)
            case .fileReadFailed(let path, let inner): compactState = .decodeError(path, inner)
            case .decodingFailed(let path, let inner): compactState = .decodeError(path, inner)
            }
        } catch {
            compactState = .decodeError(url.path, error)
        }
    }

    private func loadCatalog() {
        guard let url = catalogURL else { return }
        catalogForValidation = try? YAMLParser.loadAgentCatalog(from: url)
    }

    // MARK: - File Pickers

    private func openWorkflowFilePicker() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            SecurityScopedAccess.remember(url: url, kind: .workflowSource)
            overrideWorkflowURL = url
            loadFull()
        }
        #endif
    }

    private func openCompactFilePicker() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            SecurityScopedAccess.remember(url: url, kind: .workflowSource)
            overrideCompactURL = url
            loadCompact()
        }
        #endif
    }

    private func rawExcerpt(at path: String) -> String? {
        guard let content = try? SecurityScopedAccess.loadString(from: URL(fileURLWithPath: path)) else {
            return nil
        }
        let lines = content.components(separatedBy: .newlines)
        return lines.prefix(50).joined(separator: "\n")
    }
}
