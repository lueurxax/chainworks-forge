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
    let catalogURL: URL?

    private var effectiveURL: URL? { overrideURL ?? catalogURL }

    var body: some View {
        NavigationSplitView {
            switch state {
            case .loading:
                ProgressView("Loading agents.yaml...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

            case .loaded(let catalog, let issues):
                VStack(spacing: 0) {
                    summaryStrip(catalog: catalog, issues: issues)
                    List {
                        // Agent list
                        ForEach(catalog.agents) { agent in
                            NavigationLink {
                                agentDetail(agent, catalog: catalog)
                            } label: {
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
                VStack(spacing: 12) {
                    ContentUnavailableView(
                        "File Not Found",
                        systemImage: "doc.questionmark",
                        description: Text(path)
                    )
                    Button("Open File\u{2026}") { openFilePicker() }
                        .buttonStyle(.bordered)
                }

            case .decodeError(let path, let error):
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle).foregroundStyle(.red)
                    Text("Decode Error").font(.headline)
                    Text(path).font(.caption).foregroundStyle(.secondary)
                    ScrollView {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(error.localizedDescription)
                                .font(.system(.body, design: .monospaced))
                                .textSelection(.enabled)

                            if let rawContent = rawExcerpt(at: path) {
                                Divider()
                                Text("Raw YAML excerpt:")
                                    .font(.caption).foregroundStyle(.secondary)
                                Text(rawContent)
                                    .font(.system(.caption, design: .monospaced))
                                    .textSelection(.enabled)
                            }
                        }
                        .padding()
                    }
                    Button("Reload") { loadCatalog() }
                }
                .padding()
            }
        } detail: {
            Text("Select an agent")
                .foregroundStyle(.secondary)
        }
        .navigationSplitViewColumnWidth(min: 200, ideal: 250)
        .task { loadCatalog() }
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
            Section("Skill") {
                LabeledContent("Ref", value: agent.skillRef)
                if let role = agent.skillRole {
                    LabeledContent("Role", value: role)
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
        } catch let error as YAMLParserError {
            switch error {
            case .fileNotFound(let path):
                state = .fileNotFound(path)
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
            overrideURL = url
            loadCatalog()
        }
        #endif
    }

    private func rawExcerpt(at path: String) -> String? {
        guard let data = FileManager.default.contents(atPath: path),
              let content = String(data: data, encoding: .utf8) else {
            return nil
        }
        let lines = content.components(separatedBy: .newlines)
        return lines.prefix(50).joined(separator: "\n")
    }
}
