import SwiftUI

enum LoadState<T: Sendable>: Sendable {
    case loading
    case loaded(T, [ValidationIssue])
    case fileNotFound(String)
    case decodeError(String, Error)
}

struct AgentCatalogView: View {
    @State private var state: LoadState<AgentCatalog> = .loading
    @State private var overrideURL: URL?
    @State private var selectedAgentID: String?
    let catalogURL: URL?
    let initialSelectedAgentID: String?
    let selectionState: Binding<String?>?

    private var effectiveURL: URL? { overrideURL ?? catalogURL }

    init(
        catalogURL: URL?,
        initialSelectedAgentID: String? = nil,
        selectionState: Binding<String?>? = nil
    ) {
        self.catalogURL = catalogURL
        self.initialSelectedAgentID = initialSelectedAgentID
        self.selectionState = selectionState
    }

    var body: some View {
        NavigationSplitView {
            switch state {
            case .loading:
                VStack(alignment: .leading, spacing: 14) {
                    ForEach(0..<8) { _ in
                        VStack(alignment: .leading, spacing: 6) {
                            ForgeSkeleton.headline(width: 150)
                            ForgeSkeleton.text(width: 100)
                        }
                        .padding(.vertical, 4)
                    }
                }
                .padding()
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

            case .loaded(let catalog, let issues):
                VStack(spacing: 0) {
                    if let selectedAgent = resolvedSelectedAgent(in: catalog) {
                        Color.clear
                            .frame(width: 1, height: 1)
                            .accessibilityIdentifier("agent-catalog-selected-\(selectedAgent.id)")
                    }
                    summaryStrip(catalog: catalog, issues: issues)
                    List(selection: $selectedAgentID) {
                        ForEach(groupedAgents, id: \.0) { group, agents in
                            Section(group) {
                                ForEach(agents) { agent in
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(agent.title).font(.headline)
                                        Text(agent.id).font(.caption).foregroundStyle(.secondary)
                                        HStack(spacing: 8) {
                                            Label(agent.backendProfile, systemImage: "server.rack")
                                            Label(agent.permissionProfile, systemImage: "lock.shield")
                                        }
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                    }
                                    .accessibilityIdentifier("agent-catalog-agent-\(agent.id)")
                                    .tag(Optional(agent.id))
                                }
                            }
                        }

                        // Validation issues section
                        if !issues.isEmpty {
                            Section("Validation Issues (\(issues.count))") {
                                ForEach(issues) { issue in
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
                        }
                    }
                }

            case .fileNotFound(let path):
                ForgeEmptyState(
                    title: "File Not Found",
                    systemImage: "doc.questionmark",
                    description: path,
                    actionTitle: "Open File\u{2026}",
                    action: { openFilePicker() }
                )

            case .decodeError(let path, let error):
                ForgeEmptyState(
                    title: "Decode Error",
                    systemImage: "exclamationmark.triangle",
                    description: "\(path)\n\n\(error.localizedDescription)",
                    actionTitle: "Retry",
                    action: { loadCatalog() }
                )
            }
        } detail: {
            switch state {
            case .loaded(let catalog, _):
                if let selectedAgent = resolvedSelectedAgent(in: catalog) {
                    VStack(alignment: .leading, spacing: 0) {
                        Color.clear
                            .frame(width: 1, height: 1)
                            .accessibilityIdentifier("agent-catalog-selected-\(selectedAgent.id)")
                        agentDetail(selectedAgent, catalog: catalog)
                    }
                } else {
                    Text("Select an agent")
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("agent-catalog-detail-placeholder")
                }

            case .loading, .fileNotFound, .decodeError:
                Text("Select an agent")
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("agent-catalog-detail-placeholder")
            }
        }
        .navigationSplitViewColumnWidth(min: 200, ideal: 250)
        .accessibilityIdentifier("agent-catalog-view")
        .onChange(of: selectedAgentID) {
            selectionState?.wrappedValue = selectedAgentID
        }
        .task { loadCatalog() }
    }

    private var groupedAgents: [(String, [AgentDefinition])] {
        guard case .loaded(let catalog, _) = state else { return [] }
        return catalog.groupedAgents().map { ($0.label, $0.agents) }
    }

    private func summaryStrip(catalog: AgentCatalog, issues: [ValidationIssue]) -> some View {
        let errorCount = issues.filter { $0.severity == .error }.count
        let warnCount = issues.filter { $0.severity == .warning }.count
        let color: Color = errorCount > 0 ? .red : (warnCount > 0 ? .yellow : .green)

        return HStack {
            Image(systemName: errorCount > 0 ? "xmark.circle.fill" : "checkmark.circle.fill")
                .foregroundStyle(color)
            Text("\(catalog.agents.count) agents \u{00B7} \(catalog.backendProfiles.count) backends \u{00B7} \(catalog.permissionProfiles.count) permissions")
                .accessibilityIdentifier("agent-catalog-count")
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

    private func agentDetail(_ agent: AgentDefinition, catalog: AgentCatalog) -> some View {
        Form {
            Section("Identity") {
                LabeledContent("ID", value: agent.id)
                LabeledContent("Title", value: agent.title)
                LabeledContent("Mode", value: agent.mode)
            }
            Section("Backend") {
                LabeledContent("Profile", value: agent.backendProfile)
                if let profile = catalog.backendProfiles[agent.backendProfile] {
                    LabeledContent("Provider", value: profile.provider)
                    LabeledContent("Model", value: profile.model)
                    LabeledContent("Effort", value: profile.effort)
                    LabeledContent("Max Turns", value: "\(profile.maxTurns)")
                }
            }
            Section("Permissions") {
                LabeledContent("Profile", value: agent.permissionProfile)
            }
            if agent.xcodeBrokerRequired == true
                || agent.xcodeShimInjectionSignal == true
                || agent.requiresXcodeHostExecution == true
            {
                Section("Infrastructure") {
                    if let required = agent.xcodeBrokerRequired {
                        LabeledContent("Xcode Broker Required", value: required ? "Yes" : "No")
                            .accessibilityIdentifier("infrastructure-xcode-broker-required")
                    }
                    if let signal = agent.xcodeShimInjectionSignal {
                        LabeledContent("Xcode Shim Injection", value: signal ? "Yes" : "No")
                            .accessibilityIdentifier("infrastructure-xcode-shim-injection-signal")
                    }
                    if let hostExecution = agent.requiresXcodeHostExecution {
                        LabeledContent("Host Xcode Execution", value: hostExecution ? "Yes" : "No")
                            .accessibilityIdentifier("infrastructure-requires-xcode-host-execution")
                    }
                    if agent.xcodeBrokerRequired == true {
                        Text("First use may require one Xcode consent interaction.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            Section("Skill") {
                LabeledContent("Ref", value: agent.skillRef)
                if let role = agent.skillRole {
                    LabeledContent("Role", value: role)
                }
                if let resolvedSkill = resolveSkill(for: agent, catalog: catalog) {
                    Color.clear
                        .frame(width: 1, height: 1)
                        .accessibilityIdentifier("agent-catalog-skill-section-\(agent.id)")
                    LabeledContent("Type", value: resolvedSkill.type.catalogType)
                    if let sourcePath = resolvedSkill.sourcePath {
                        LabeledContent("Source Path", value: sourcePath)
                    } else if let sourceDescription = resolvedSkill.sourceDescription {
                        LabeledContent("Source", value: sourceDescription)
                    }
                    LabeledContent("Content Hash", value: resolvedSkill.contentHash)
                    LabeledContent("Injected Hash", value: resolvedSkill.injectedContentHash)
                    if let summary = resolvedSkill.specializationSummary {
                        LabeledContent("Specialization", value: summary)
                    }
                    if let manifest = resolvedSkill.bundleManifest, manifest.hasCompanions {
                        LabeledContent(
                            "Bundle Companions",
                            value: "\(manifest.references.count) refs, \(manifest.assets.count) assets, \(manifest.evals.count) evals, \(manifest.agents.count) agents"
                        )
                    }
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Content Preview")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text(resolvedSkill.contentSummary)
                            .font(.system(.body, design: .monospaced))
                            .textSelection(.enabled)
                            .accessibilityIdentifier("agent-catalog-skill-preview-\(agent.id)")
                    }
                } else if let error = resolveSkillError(for: agent, catalog: catalog) {
                    Text(error.localizedDescription)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
            Section("Inputs") {
                if agent.inputs.isEmpty {
                    Text("None").foregroundStyle(.secondary)
                } else {
                    ForEach(agent.inputs, id: \.self) { input in
                        Text(input).font(.system(.body, design: .monospaced))
                    }
                }
            }
            Section("Outputs") {
                if agent.outputs.isEmpty {
                    Text("None").foregroundStyle(.secondary)
                } else {
                    ForEach(agent.outputs, id: \.self) { output in
                        Text(output).font(.system(.body, design: .monospaced))
                    }
                }
            }
            if let contract = agent.outputContract {
                Section("Output Contract") {
                    LabeledContent("Contract", value: contract)
                }
            }
            Section("Prompt") {
                Text(agent.prompt)
                    .font(.system(.body, design: .monospaced))
                    .textSelection(.enabled)
            }
        }
        .formStyle(.grouped)
        .navigationTitle(agent.title)
    }

    private func loadCatalog() {
        guard let url = effectiveURL else {
            state = .fileNotFound("No catalog URL configured")
            return
        }
        do {
            let catalog = try YAMLParser.loadAgentCatalog(from: url)
            let issues = YAMLValidator.validateBackendProfileRefs(catalog)
                + YAMLValidator.validatePermissionProfileRefs(catalog)
                + YAMLValidator.validateSkillRefs(catalog)
                + YAMLValidator.validateOutputContractRefs(catalog)
                + YAMLValidator.validateArtifactRefs(catalog)
                + YAMLValidator.validateEnvPlaceholders(catalog)
            state = .loaded(catalog, issues)
            selectedAgentID = resolvedInitialSelection(in: catalog)
            selectionState?.wrappedValue = selectedAgentID
        } catch let error as YAMLParserError {
            switch error {
            case .fileNotFound(let path):
                state = .fileNotFound(path)
            case .fileReadFailed(let path, let inner):
                state = .decodeError(path, inner)
            case .decodingFailed(let path, let inner):
                state = .decodeError(path, inner)
            }
        } catch {
            state = .decodeError(url.path, error)
        }
    }

    private func openFilePicker() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            SecurityScopedAccess.remember(url: url, kind: .catalogSource)
            overrideURL = url
            loadCatalog()
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

    private func resolvedInitialSelection(in catalog: AgentCatalog) -> String? {
        if let selectedAgentID,
           catalog.agents.contains(where: { $0.id == selectedAgentID }) {
            return selectedAgentID
        }
        if let initialSelectedAgentID,
           catalog.agents.contains(where: { $0.id == initialSelectedAgentID }) {
            return initialSelectedAgentID
        }
        return catalog.agents.first?.id
    }

    private func resolvedSelectedAgent(in catalog: AgentCatalog) -> AgentDefinition? {
        guard let selectedAgentID else { return nil }
        return catalog.agents.first(where: { $0.id == selectedAgentID })
    }

    private func resolveSkill(for agent: AgentDefinition, catalog: AgentCatalog) -> ResolvedSkill? {
        guard let skillRef = catalog.skills[agent.skillRef] else { return nil }
        let context = SkillResolverContext(catalogBaseURL: effectiveURL)
        return try? SkillResolver.resolve(
            skillID: agent.skillRef,
            skillRef: skillRef,
            skillRole: agent.skillRole,
            context: context
        )
    }

    private func resolveSkillError(for agent: AgentDefinition, catalog: AgentCatalog) -> Error? {
        guard let skillRef = catalog.skills[agent.skillRef] else { return nil }
        let context = SkillResolverContext(catalogBaseURL: effectiveURL)
        do {
            _ = try SkillResolver.resolve(
                skillID: agent.skillRef,
                skillRef: skillRef,
                skillRole: agent.skillRole,
                context: context
            )
            return nil
        } catch {
            return error
        }
    }
}
