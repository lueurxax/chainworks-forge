import SwiftUI
import SwiftData
import UniformTypeIdentifiers

struct ProviderSettingsView: View {
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry
    @Environment(GooseServerManager.self) private var gooseServerManager

    @State private var draft = ProviderDraft()
    @State private var secret = ""
    @State private var importMessage: String?
    @State private var exportPath: String?
    @State private var showWizard = false
    @State private var selectedAssistantProviderID: UUID?
    @State private var availableModelsByProviderID: [UUID: [String]] = [:]
    @State private var gooseTroubleshootingByProviderID: [UUID: ProviderTroubleshootingReport] = [:]
    private let showsUITestReadyMarker = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] != nil

    @State private var showAddProviderSheet = false
    @State private var isRefreshing = false
    @State private var refreshError: String?

    var body: some View {
        NavigationStack {
            List {
                if showsUITestReadyMarker {
                    Section {
                        Button("Provider Settings Ready") {}
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("provider-settings-surface-ready")
                    }
                }
                Section {
                    Text("Provider Settings")
                        .font(.title2.bold())
                        .accessibilityIdentifier("provider-settings-title")
                    Text("Use Goose-backed setup first for Codex and Claude. Treat raw paths and storage as advanced configuration, not the primary setup journey.")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                    Button("Open First Run Wizard") {
                        showWizard = true
                    }
                    .accessibilityIdentifier("provider-settings-open-wizard")
                }
                managedGooseServerSection
                configuredProvidersSection
                // Proposal 012 (H-01): Transfer moved inline as secondary section
                transferSection
                // Proposal 012 (H-01): Advanced config behind DisclosureGroup
                advancedConfigurationSection
            }
            .navigationTitle("Provider Settings")
            .accessibilityIdentifier("provider-settings-view")
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    // Proposal 012 (H-01): Add Provider moved to toolbar +
                    Button {
                        showAddProviderSheet = true
                    } label: {
                        Label("Add Provider", systemImage: "plus")
                    }
                    .accessibilityIdentifier("provider-settings-add-provider-toolbar")
                    Button {
                        Task {
                            isRefreshing = true
                            refreshError = nil
                            await refreshDiagnostics()
                            isRefreshing = false
                        }
                    } label: {
                        if isRefreshing {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Label("Refresh", systemImage: "arrow.clockwise")
                        }
                    }
                    .disabled(isRefreshing)
                    .accessibilityIdentifier("provider-settings-refresh-health")
                    Button("First Run Wizard") {
                        showWizard = true
                    }
                    .accessibilityIdentifier("provider-settings-open-wizard")
                }
            }
            .task {
                await refreshDiagnostics()
            }
            .sheet(isPresented: $showWizard) {
                FirstRunSetupWizard(isPresented: $showWizard)
                    .environment(executionService)
                    .environment(appConfigurationStore)
                    .environment(providerSettingsStore)
                    .environment(providerRegistry)
                    .environment(gooseServerManager)
            }
            .sheet(isPresented: Binding(
                get: { selectedAssistantProviderID != nil },
                set: { if !$0 { selectedAssistantProviderID = nil } }
            )) {
                if let selectedAssistantProviderID {
                    GooseProviderConnectionAssistantView(
                        providerID: selectedAssistantProviderID,
                        origin: .providerSettings
                    )
                        .environment(appConfigurationStore)
                        .environment(providerSettingsStore)
                        .environment(providerRegistry)
                        .environment(gooseServerManager)
                }
            }
            // Proposal 012 (H-01): Add Provider sheet
            .sheet(isPresented: $showAddProviderSheet) {
                addProviderSheet
            }
        }
    }

    // MARK: - Add Provider Sheet (H-01)
    @ViewBuilder
    private var addProviderSheet: some View {
        NavigationStack {
            Form {
                Section("Provider Family") {
                    Picker("Family", selection: $draft.family) {
                        ForEach(ProviderFamily.allCases, id: \.self) { family in
                            Text(family.displayName).tag(family)
                        }
                    }
                    .accessibilityIdentifier("provider-settings-family-picker")
                    .onChange(of: draft.family) { _, newFamily in
                        draft.applyFamilyDefaults(newFamily, configuration: appConfigurationStore.configuration)
                    }
                }

                Section("Transport & Auth") {
                    Picker("Transport", selection: $draft.transport) {
                        ForEach(ProviderTransport.allCases, id: \.self) { transport in
                            Text(transport.displayName).tag(transport)
                        }
                    }
                    .accessibilityIdentifier("provider-settings-transport-picker")
                    Text(draft.transport == .gooseServer
                         ? "Goose Server uses the same runtime path as live runs."
                         : "Choose the transport that matches how the provider is actually reached.")
                        .font(DesignTokens.Typography.micro)
                        .foregroundStyle(.secondary)

                    Picker("Auth", selection: $draft.authMode) {
                        ForEach(ProviderAuthMode.allCases, id: \.self) { authMode in
                            Text(authMode.rawValue).tag(authMode)
                        }
                    }
                    .accessibilityIdentifier("provider-settings-auth-picker")
                }

                Section("Details") {
                    TextField("Display Name", text: $draft.displayName)
                        .accessibilityIdentifier("provider-settings-display-name")
                    TextField("Default Model", text: $draft.defaultModel)
                        .accessibilityIdentifier("provider-settings-default-model")
                    TextField(draft.transport == .gooseServer ? "Goose Endpoint (required)" : "Endpoint (optional)", text: $draft.endpoint)
                        .accessibilityIdentifier("provider-settings-endpoint")
                    if draft.authMode != .none {
                        SecureField("Secret", text: $secret)
                            .accessibilityIdentifier("provider-settings-secret")
                    }
                }
            }
            .navigationTitle("Add Provider")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { showAddProviderSheet = false }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        saveDraft()
                        showAddProviderSheet = false
                    }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("provider-settings-save-provider")
                }
            }
        }
        .frame(minWidth: 480, minHeight: 400)
    }

    // Proposal 012 (H-01): Advanced Configuration behind DisclosureGroup
    private var advancedConfigurationSection: some View {
        Section {
            DisclosureGroup("Advanced Configuration") {
            TextField("Run Storage Base Path", text: Binding(
                get: { appConfigurationStore.configuration.runStorageBasePath },
                set: { newValue in
                    appConfigurationStore.update { $0.runStorageBasePath = newValue }
                }
            ))
            .accessibilityIdentifier("provider-settings-run-storage-path")

            TextField("Workflow Source Path", text: Binding(
                get: { appConfigurationStore.configuration.workflowSourcePath },
                set: { newValue in
                    appConfigurationStore.update { $0.workflowSourcePath = newValue }
                }
            ))
            .accessibilityIdentifier("provider-settings-workflow-path")

            TextField("Agent Catalog Source Path", text: Binding(
                get: { appConfigurationStore.configuration.agentCatalogSourcePath },
                set: { newValue in
                    appConfigurationStore.update { $0.agentCatalogSourcePath = newValue }
                }
            ))
            .accessibilityIdentifier("provider-settings-catalog-path")

            TextField("Worktree Base Path", text: Binding(
                get: { appConfigurationStore.configuration.worktreeBasePath ?? "" },
                set: { newValue in
                    appConfigurationStore.update { $0.worktreeBasePath = newValue.isEmpty ? nil : newValue }
                }
            ))
            .accessibilityIdentifier("provider-settings-worktree-path")

            TextField("Support Bundle Export Path", text: Binding(
                get: { appConfigurationStore.configuration.supportBundleExportPath ?? "" },
                set: { newValue in
                    appConfigurationStore.update { $0.supportBundleExportPath = newValue.isEmpty ? nil : newValue }
                }
            ))
            .accessibilityIdentifier("provider-settings-export-path")

            Text("Configuration Source: \(appConfigurationStore.configuration.activeConfigurationSource.displayName)")
                .font(.caption)
                .foregroundStyle(.secondary)

            Toggle("Notify on Provider Failure", isOn: Binding(
                get: { providerSettingsStore.settings.notificationOnProviderFailure },
                set: { newValue in
                    providerSettingsStore.replace(with: ProviderSettings(
                        configuredProviders: providerSettingsStore.settings.configuredProviders,
                        preferredProviderIDsByFamily: providerSettingsStore.settings.preferredProviderIDsByFamily,
                        notificationOnProviderFailure: newValue,
                        runStartRequiresCleanPreflight: providerSettingsStore.settings.runStartRequiresCleanPreflight
                    ))
                }
            ))
            .accessibilityIdentifier("provider-settings-notify-on-failure")

            Toggle("Require Clean Preflight Before Run Start", isOn: Binding(
                get: { providerSettingsStore.settings.runStartRequiresCleanPreflight },
                set: { newValue in
                    providerSettingsStore.replace(with: ProviderSettings(
                        configuredProviders: providerSettingsStore.settings.configuredProviders,
                        preferredProviderIDsByFamily: providerSettingsStore.settings.preferredProviderIDsByFamily,
                        notificationOnProviderFailure: providerSettingsStore.settings.notificationOnProviderFailure,
                        runStartRequiresCleanPreflight: newValue
                    ))
                }
            ))
            .accessibilityIdentifier("provider-settings-require-clean-preflight")
            }
        }
    }

    // Proposal 012 (H-01): Configured providers with GroupBox boundaries
    private var configuredProvidersSection: some View {
        Section("Configured Providers") {
            if providerSettingsStore.settings.configuredProviders.isEmpty {
                Text("No providers configured yet. Use the + button to add one.")
                    .foregroundStyle(.secondary)
            }

            // Proposal 012 (L-12): Inline refresh error
            if let refreshError {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(DesignTokens.Status.warning)
                    Text(refreshError)
                        .font(DesignTokens.Typography.supporting)
                    Spacer()
                    Button("Retry") {
                        Task {
                            self.refreshError = nil
                            isRefreshing = true
                            await refreshDiagnostics()
                            isRefreshing = false
                        }
                    }
                    .buttonStyle(.borderless)
                }
            }

            ForEach(providerSettingsStore.settings.configuredProviders) { provider in
                // Proposal 012 (H-01): Each provider in a GroupBox
                GroupBox {
                    VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
                        HStack {
                            Text(provider.displayName)
                                .font(DesignTokens.Typography.cardTitle)
                            Spacer()
                            StatusCapsule(
                                text: provider.family.displayName,
                                color: .blue,
                                size: .small
                            )
                        }

                        Text("\(provider.transport.displayName) · \(provider.defaultModel ?? "default model not set")")
                            .font(DesignTokens.Typography.supporting)
                            .foregroundStyle(.secondary)

                        Text(capabilitySummary(for: provider.capabilities))
                            .font(DesignTokens.Typography.micro)
                            .foregroundStyle(.tertiary)

                        if provider.family.gooseFirstPreferred && provider.transport != .gooseServer {
                            Label("Goose-first setup is preferred for this family", systemImage: "info.circle")
                                .font(DesignTokens.Typography.micro)
                                .foregroundStyle(.blue)
                        }

                        // Proposal 012 (L-12): Inline health with distinct states
                        if let snapshot = providerRegistry.healthSnapshot(for: provider.id) {
                            Divider()
                            Label(snapshot.summary, systemImage: healthIcon(snapshot.status))
                                .font(DesignTokens.Typography.micro)
                                .foregroundStyle(color(for: snapshot.status))
                            if !snapshot.blockingIssues.isEmpty {
                                Text(snapshot.blockingIssues.joined(separator: " • "))
                                    .font(DesignTokens.Typography.micro)
                                    .foregroundStyle(DesignTokens.Status.error)
                            }
                            Text("Checked \(snapshot.checkedAt.formatted(date: .omitted, time: .shortened))")
                                .font(DesignTokens.Typography.micro)
                                .foregroundStyle(.tertiary)
                        }

                        if let models = availableModelsByProviderID[provider.id], !models.isEmpty {
                            Text("Models: \(models.joined(separator: ", "))")
                                .font(DesignTokens.Typography.micro)
                                .foregroundStyle(.secondary)
                        }

                        if let report = gooseTroubleshootingByProviderID[provider.id] {
                            ProviderTroubleshootingPanel(report: report)
                        }

                        Divider()

                        HStack(spacing: DesignTokens.Spacing.medium) {
                            if provider.family.gooseFirstPreferred {
                                Button("Open Goose Assistant") {
                                    selectedAssistantProviderID = provider.id
                                }
                                .buttonStyle(.borderless)
                                .accessibilityIdentifier("provider-settings-open-assistant-\(provider.family.rawValue)")
                            }
                            Button("Prefer") {
                                providerSettingsStore.setPreferredProvider(id: provider.id, for: provider.family)
                            }
                            .buttonStyle(.borderless)
                            Spacer()
                            Button("Remove", role: .destructive) {
                                providerSettingsStore.removeProvider(id: provider.id)
                            }
                            .buttonStyle(.borderless)
                        }
                    }
                }
                .padding(.vertical, DesignTokens.Spacing.compact)
            }
        }
        .onAppear {
            draft.applyFamilyDefaults(draft.family, configuration: appConfigurationStore.configuration)
        }
    }

    private var managedGooseServerSection: some View {
        Section("Managed Goose Server") {
            LabeledContent("State", value: gooseServerManager.statusSummary)
                .accessibilityIdentifier("provider-settings-goose-state")
            if let baseURL = appConfigurationStore.configuration.gooseServerBaseURL {
                LabeledContent("Base URL", value: baseURL.absoluteString)
            }
            LabeledContent("Autostart", value: appConfigurationStore.configuration.gooseServerAutostart ? "Enabled" : "Disabled")
            if let binaryPath = appConfigurationStore.configuration.gooseServerBinaryPath {
                LabeledContent("Binary", value: binaryPath)
            }

            HStack {
                Button("Start Managed Server") {
                    Task { await gooseServerManager.ensureRunning() }
                }
                .accessibilityIdentifier("provider-settings-start-goose")
                Button("Refresh Server Status") {
                    Task { await gooseServerManager.refreshStatus() }
                }
                .accessibilityIdentifier("provider-settings-refresh-goose")
            }
            .buttonStyle(.borderless)
        }
    }

    private var transferSection: some View {
        Section("Settings Transfer") {
            if let exportPath {
                Text("Last export: \(exportPath)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("provider-settings-export-message")
            }
            if let importMessage {
                Text(importMessage)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("provider-settings-import-message")
            }

            Button("Export Settings") {
                exportSettings()
            }
            .accessibilityIdentifier("provider-settings-export")

            Button("Import Settings") {
                importSettings()
            }
            .accessibilityIdentifier("provider-settings-import")
        }
    }

    private func saveDraft() {
        draft.normalizeForSave()
        let provider = draft.makeProvider()
        providerSettingsStore.upsert(provider: provider)
        if draft.authMode != .none, !secret.isEmpty {
            try? providerRegistry.secretStore.setSecret(secret, for: ProviderAdapterSupport.secretKey(for: provider))
        }
        draft = ProviderDraft()
        draft.applyFamilyDefaults(draft.family, configuration: appConfigurationStore.configuration)
        secret = ""
        Task { await refreshDiagnostics() }
    }

    private func exportSettings() {
        let transfer = SettingsTransferService(
            appConfigurationStore: appConfigurationStore,
            providerSettingsStore: providerSettingsStore,
            secretStore: providerRegistry.secretStore
        )
        do {
            exportPath = try transfer.exportSettings().path
            importMessage = nil
        } catch {
            importMessage = error.localizedDescription
        }
    }

    private func importSettings() {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.json]
        if panel.runModal() == .OK, let url = panel.url {
            let transfer = SettingsTransferService(
                appConfigurationStore: appConfigurationStore,
                providerSettingsStore: providerSettingsStore,
                secretStore: providerRegistry.secretStore
            )
            do {
                _ = try transfer.importSettings(from: url)
                importMessage = "Imported settings from \(url.lastPathComponent)"
                Task { await refreshDiagnostics() }
            } catch {
                importMessage = error.localizedDescription
            }
        }
        #endif
    }

    private func refreshDiagnostics() async {
        await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
        var models: [UUID: [String]] = [:]
        for provider in providerRegistry.configuredProviders {
            models[provider.id] = await providerRegistry.availableModels(for: provider)
        }
        availableModelsByProviderID = models
        gooseTroubleshootingByProviderID = Dictionary(
            uniqueKeysWithValues: providerRegistry.configuredProviders.compactMap { provider in
                providerRegistry.troubleshootingReport(for: provider.id).map { (provider.id, $0) }
            }
        )
    }

    private func healthIcon(_ status: ProviderStatus) -> String {
        switch status {
        case .healthy:
            return "checkmark.circle.fill"
        case .degraded:
            return "exclamationmark.triangle.fill"
        case .unavailable:
            return "xmark.circle.fill"
        case .unknown:
            return "questionmark.circle"
        }
    }

    private func color(for status: ProviderStatus) -> Color {
        switch status {
        case .healthy:
            return .green
        case .degraded:
            return .orange
        case .unavailable:
            return .red
        case .unknown:
            return .secondary
        }
    }

    private func capabilitySummary(for capabilities: ProviderCapabilities) -> String {
        var flags: [String] = []
        if capabilities.supportsStreaming { flags.append("streaming") }
        if capabilities.supportsTools { flags.append("tools") }
        if capabilities.supportsStructuredOutput { flags.append("structured") }
        if capabilities.supportsEffortControl { flags.append("effort") }
        if capabilities.supportsSessionResume { flags.append("resume") }
        if capabilities.supportsFileEditing { flags.append("file-edit") }
        if capabilities.supportsSandboxHints { flags.append("sandbox-hints") }
        return flags.isEmpty ? "Capabilities unavailable" : "Capabilities: \(flags.joined(separator: ", "))"
    }
}

#Preview("Provider Settings — Configured") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
    let gooseServerManager = GooseServerManager(appConfigurationStore: appConfigurationStore)

    return ProviderSettingsView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .environment(gooseServerManager)
        .frame(width: 1100, height: 820)
}

struct ProviderDraft {
    var family: ProviderFamily = .codex
    var displayName: String = ""
    var transport: ProviderTransport = .gooseServer
    var endpoint: String = ""
    var authMode: ProviderAuthMode = .none
    var defaultModel: String = ""

    func makeProvider() -> ConfiguredProvider {
        let fallbackName: String = {
            if family.gooseFirstPreferred && transport == .gooseServer {
                return "\(family.displayName) Goose"
            }
            return "\(family.displayName) \(transport.displayName)"
        }()
        let normalizedDefaultModel: String? = {
            guard let canonicalModel = ProviderDefaults.canonicalModel(
                defaultModel,
                for: family,
                transport: transport
            ) else {
                return nil
            }
            guard ProviderDefaults.model(canonicalModel, isCompatibleWith: family) else {
                return ProviderDefaults.defaultModel(for: family)
            }
            return canonicalModel
        }()
        return ConfiguredProvider(
            family: family,
            displayName: displayName.isEmpty ? fallbackName : displayName,
            transport: transport,
            endpoint: endpoint.isEmpty ? nil : endpoint,
            authMode: authMode,
            defaultModel: normalizedDefaultModel,
            capabilities: .default(for: family)
        )
    }

    mutating func applyFamilyDefaults(_ family: ProviderFamily, configuration: AppConfiguration) {
        let previousFamily = self.family
        let previousGeneratedName = generatedDisplayName(for: previousFamily, transport: transport)
        let previousDefaultModel = ProviderDefaults.canonicalModel(
            defaultModel,
            for: previousFamily,
            transport: transport
        ) ?? defaultModel
        let resolvedTransport: ProviderTransport
        switch family {
        case .codex, .claude:
            resolvedTransport = .gooseServer
        case .gemini:
            resolvedTransport = .httpAPI
        }

        self.family = family

        if displayName.isEmpty || displayName == previousGeneratedName {
            displayName = generatedDisplayName(for: family, transport: resolvedTransport)
        }

        transport = resolvedTransport

        let previousFamilyDefault = ProviderDefaults.defaultModel(for: previousFamily)
        if previousDefaultModel.isEmpty
            || previousDefaultModel == previousFamilyDefault
            || !ProviderDefaults.model(previousDefaultModel, isCompatibleWith: family) {
            defaultModel = ProviderDefaults.defaultModel(for: family)
        }

        if transport == .gooseServer {
            if endpoint.isEmpty {
                endpoint = configuration.gooseServerBaseURL?.absoluteString ?? ""
            }
            if authMode == .none, let secret = configuration.gooseServerSecretKey, !secret.isEmpty {
                authMode = .apiKey
            }
        }
    }

    private func generatedDisplayName(for family: ProviderFamily, transport: ProviderTransport) -> String {
        ProviderDefaults.generatedDisplayName(for: family, transport: transport)
    }

    mutating func normalizeForSave() {
        defaultModel = ProviderDefaults.canonicalModel(
            defaultModel,
            for: family,
            transport: transport
        ) ?? ""
        if !defaultModel.isEmpty, !ProviderDefaults.model(defaultModel, isCompatibleWith: family) {
            defaultModel = ProviderDefaults.defaultModel(for: family)
        }
    }
}
