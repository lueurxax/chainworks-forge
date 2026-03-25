import SwiftUI
import UniformTypeIdentifiers

struct ProviderSettingsView: View {
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry

    @State private var draft = ProviderDraft()
    @State private var secret = ""
    @State private var importMessage: String?
    @State private var exportPath: String?
    @State private var showWizard = false
    @State private var availableModelsByProviderID: [UUID: [String]] = [:]
    private let showsUITestReadyMarker = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] != nil

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
                        .font(.title3.bold())
                        .accessibilityIdentifier("provider-settings-title")
                    Button("Open First Run Wizard") {
                        showWizard = true
                    }
                    .accessibilityIdentifier("provider-settings-open-wizard")
                }
                configurationSection
                providerSection
                transferSection
            }
            .navigationTitle("Provider Settings")
            .accessibilityIdentifier("provider-settings-view")
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    Button("Refresh Health") {
                        Task { await refreshDiagnostics() }
                    }
                    .accessibilityIdentifier("provider-settings-refresh-health")
                    Button("Export Settings") {
                        exportSettings()
                    }
                    .accessibilityIdentifier("provider-settings-toolbar-export")
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
            }
        }
    }

    private var configurationSection: some View {
        Section("App Configuration") {
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

    private var providerSection: some View {
        Section("Configured Providers") {
            if providerSettingsStore.settings.configuredProviders.isEmpty {
                Text("No providers configured yet")
                    .foregroundStyle(.secondary)
            }

            ForEach(providerSettingsStore.settings.configuredProviders) { provider in
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(provider.displayName)
                            .font(.headline)
                        Spacer()
                        Text(provider.family.displayName)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    Text("\(provider.transport.rawValue) · \(provider.defaultModel ?? "default model not set")")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Text(capabilitySummary(for: provider.capabilities))
                        .font(.caption2)
                        .foregroundStyle(.tertiary)

                    if let snapshot = providerRegistry.healthSnapshot(for: provider.id) {
                        Label(snapshot.summary, systemImage: healthIcon(snapshot.status))
                            .font(.caption2)
                            .foregroundStyle(color(for: snapshot.status))
                        if !snapshot.blockingIssues.isEmpty {
                            Text(snapshot.blockingIssues.joined(separator: " • "))
                                .font(.caption2)
                                .foregroundStyle(.red)
                        }
                        Text("Checked \(snapshot.checkedAt.formatted(date: .omitted, time: .shortened))")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                    if let models = availableModelsByProviderID[provider.id], !models.isEmpty {
                        Text("Models: \(models.joined(separator: ", "))")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }

                    HStack {
                        Button("Prefer") {
                            providerSettingsStore.setPreferredProvider(id: provider.id, for: provider.family)
                        }
                        Button("Remove", role: .destructive) {
                            providerSettingsStore.removeProvider(id: provider.id)
                        }
                    }
                    .buttonStyle(.borderless)
                }
                .padding(.vertical, 4)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Add Provider")
                    .font(.subheadline.bold())

                Picker("Family", selection: $draft.family) {
                    ForEach(ProviderFamily.allCases, id: \.self) { family in
                        Text(family.displayName).tag(family)
                    }
                }
                .accessibilityIdentifier("provider-settings-family-picker")

                Picker("Transport", selection: $draft.transport) {
                    ForEach(ProviderTransport.allCases, id: \.self) { transport in
                        Text(transport.rawValue).tag(transport)
                    }
                }
                .accessibilityIdentifier("provider-settings-transport-picker")

                Picker("Auth", selection: $draft.authMode) {
                    ForEach(ProviderAuthMode.allCases, id: \.self) { authMode in
                        Text(authMode.rawValue).tag(authMode)
                    }
                }
                .accessibilityIdentifier("provider-settings-auth-picker")

                TextField("Display Name", text: $draft.displayName)
                    .accessibilityIdentifier("provider-settings-display-name")
                TextField("Default Model", text: $draft.defaultModel)
                    .accessibilityIdentifier("provider-settings-default-model")
                TextField("Endpoint (optional)", text: $draft.endpoint)
                    .accessibilityIdentifier("provider-settings-endpoint")
                if draft.authMode != .none {
                    SecureField("Secret", text: $secret)
                        .accessibilityIdentifier("provider-settings-secret")
                }

                Button("Save Provider") {
                    saveDraft()
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("provider-settings-save-provider")
            }
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
        let provider = draft.makeProvider()
        providerSettingsStore.upsert(provider: provider)
        if draft.authMode != .none, !secret.isEmpty {
            try? providerRegistry.secretStore.setSecret(secret, for: ProviderAdapterSupport.secretKey(for: provider))
        }
        draft = ProviderDraft()
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
        await providerRegistry.refreshHealth()
        var models: [UUID: [String]] = [:]
        for provider in providerRegistry.configuredProviders {
            models[provider.id] = await providerRegistry.availableModels(for: provider)
        }
        availableModelsByProviderID = models
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

struct ProviderDraft {
    var family: ProviderFamily = .codex
    var displayName: String = ""
    var transport: ProviderTransport = .cli
    var endpoint: String = ""
    var authMode: ProviderAuthMode = .none
    var defaultModel: String = ""

    func makeProvider() -> ConfiguredProvider {
        ConfiguredProvider(
            family: family,
            displayName: displayName.isEmpty ? "\(family.displayName) \(transport.rawValue)" : displayName,
            transport: transport,
            endpoint: endpoint.isEmpty ? nil : endpoint,
            authMode: authMode,
            defaultModel: defaultModel.isEmpty ? nil : defaultModel,
            capabilities: .default(for: family)
        )
    }
}
