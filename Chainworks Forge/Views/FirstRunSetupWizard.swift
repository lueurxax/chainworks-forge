import SwiftUI
import SwiftData

struct FirstRunSetupWizard: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry

    @Binding var isPresented: Bool

    @State private var runStorageBasePath = ""
    @State private var worktreeBasePath = ""
    @State private var workflowSourcePath = ""
    @State private var agentCatalogSourcePath = ""
    @State private var supportBundleExportPath = ""
    @State private var wizardMessage: String?
    @State private var latestPreflight: PreflightReport?
    @State private var transferMessage: String?
    @State private var showPreflightReport = false
    private let showsUITestReadyMarker = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] != nil

    var body: some View {
        NavigationStack {
            Form {
                if showsUITestReadyMarker {
                    Section {
                        Button("First Run Setup Ready") {}
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("first-run-setup-surface-ready")
                    }
                }
                Section("Workspace & YAML") {
                    TextField("Run Storage Base Path", text: $runStorageBasePath)
                        .accessibilityIdentifier("first-run-run-storage-path")
                    TextField("Worktree Base Path", text: $worktreeBasePath)
                        .accessibilityIdentifier("first-run-worktree-path")
                    TextField("Workflow Source Path", text: $workflowSourcePath)
                        .accessibilityIdentifier("first-run-workflow-path")
                    TextField("Agent Catalog Source Path", text: $agentCatalogSourcePath)
                        .accessibilityIdentifier("first-run-catalog-path")
                    TextField("Support Bundle Export Path", text: $supportBundleExportPath)
                        .accessibilityIdentifier("first-run-export-path")
                }

                Section("Suggested Providers") {
                    Button("Add Codex CLI") {
                        providerSettingsStore.upsert(provider: ConfiguredProvider(
                            family: .codex,
                            displayName: "Codex CLI",
                            transport: .cli,
                            authMode: .none,
                            defaultModel: "gpt-5-codex"
                        ))
                    }
                    .accessibilityIdentifier("first-run-add-codex")
                    Button("Add Claude CLI") {
                        providerSettingsStore.upsert(provider: ConfiguredProvider(
                            family: .claude,
                            displayName: "Claude CLI",
                            transport: .cli,
                            authMode: .none,
                            defaultModel: "claude-sonnet-4"
                        ))
                    }
                    .accessibilityIdentifier("first-run-add-claude")
                    Button("Add Gemini API") {
                        providerSettingsStore.upsert(provider: ConfiguredProvider(
                            family: .gemini,
                            displayName: "Gemini API",
                            transport: .httpAPI,
                            endpoint: "https://generativelanguage.googleapis.com",
                            authMode: .apiKey,
                            defaultModel: "gemini-2.5-pro"
                        ))
                    }
                    .accessibilityIdentifier("first-run-add-gemini")

                    Text("\(providerSettingsStore.settings.configuredProviders.count) provider(s) configured")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("first-run-provider-count")
                }

                Section("Verification") {
                    Button("Refresh Provider Health") {
                        Task {
                            await providerRegistry.refreshHealth()
                            await refreshPreflight()
                        }
                    }
                    .accessibilityIdentifier("first-run-refresh-health")

                    if let latestPreflight {
                        LabeledContent("Preflight", value: latestPreflight.status.rawValue.capitalized)
                            .accessibilityIdentifier("first-run-preflight-status")
                        LabeledContent("Configuration Source", value: latestPreflight.configurationSource.displayName)
                            .accessibilityIdentifier("first-run-configuration-source")
                        if let issue = latestPreflight.blockingIssues.first {
                            Text(issue)
                                .font(.caption)
                                .foregroundStyle(.red)
                                .accessibilityIdentifier("first-run-preflight-issue")
                        } else if let warning = latestPreflight.warnings.first {
                            Text(warning)
                                .font(.caption)
                                .foregroundStyle(.orange)
                                .accessibilityIdentifier("first-run-preflight-warning")
                        } else {
                            Text("Configuration is ready for a sample run.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("first-run-preflight-ready")
                        }
                    } else {
                        Text("Save configuration, then verify provider and workspace readiness.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("first-run-preflight-placeholder")
                    }

                    if latestPreflight != nil {
                        Button("View Preflight Report") {
                            showPreflightReport = true
                        }
                        .accessibilityIdentifier("first-run-view-preflight-report")
                    }
                }

                Section("Settings Transfer") {
                    Button("Export Settings") {
                        exportSettings()
                    }
                    .accessibilityIdentifier("first-run-export-settings")

                    Button("Import Latest Settings") {
                        importLatestSettings()
                    }
                    .accessibilityIdentifier("first-run-import-settings")

                    if let transferMessage {
                        Text(transferMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("first-run-transfer-message")
                    }
                }

                Section("Sample Run Path") {
                    Text("Creates a sample idea and launches the current safe workflow path with frozen provider bindings.")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Button("Save and Launch Sample Run") {
                        Task { await saveAndLaunchSampleRun() }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(providerSettingsStore.settings.configuredProviders.isEmpty)
                    .accessibilityIdentifier("first-run-launch-sample-run")

                    if let wizardMessage {
                        Text(wizardMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("first-run-message")
                    }
                }
            }
            .navigationTitle("First Run Setup")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { isPresented = false }
                        .accessibilityIdentifier("first-run-close")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        persistConfiguration()
                        Task {
                            await providerRegistry.refreshHealth()
                            await refreshPreflight()
                        }
                        isPresented = false
                    }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("first-run-save")
                }
            }
            .task {
                runStorageBasePath = appConfigurationStore.configuration.runStorageBasePath
                worktreeBasePath = appConfigurationStore.configuration.worktreeBasePath ?? ""
                workflowSourcePath = appConfigurationStore.configuration.workflowSourcePath
                agentCatalogSourcePath = appConfigurationStore.configuration.agentCatalogSourcePath
                supportBundleExportPath = appConfigurationStore.configuration.supportBundleExportPath ?? ""
                await refreshPreflight()
            }
            .sheet(isPresented: $showPreflightReport) {
                if let latestPreflight {
                    NavigationStack {
                        PreflightReportView(report: latestPreflight)
                            .toolbar {
                                ToolbarItem(placement: .cancellationAction) {
                                    Button("Done") { showPreflightReport = false }
                                }
                            }
                    }
                    .frame(minWidth: 520, minHeight: 420)
                }
            }
        }
        .frame(minWidth: 520, minHeight: 420)
        .accessibilityIdentifier("first-run-setup-wizard")
    }

    private func persistConfiguration() {
        appConfigurationStore.update {
            $0.runStorageBasePath = runStorageBasePath
            $0.worktreeBasePath = worktreeBasePath.isEmpty ? nil : worktreeBasePath
            $0.workflowSourcePath = workflowSourcePath
            $0.agentCatalogSourcePath = agentCatalogSourcePath
            $0.supportBundleExportPath = supportBundleExportPath.isEmpty ? nil : supportBundleExportPath
            $0.activeConfigurationSource = .persistedSettings
        }
    }

    private func refreshPreflight() async {
        let workflowURL = URL(fileURLWithPath: workflowSourcePath.isEmpty ? appConfigurationStore.configuration.workflowSourcePath : workflowSourcePath)
        let catalogURL = URL(fileURLWithPath: agentCatalogSourcePath.isEmpty ? appConfigurationStore.configuration.agentCatalogSourcePath : agentCatalogSourcePath)
        let preflight = PreflightService(
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )
        latestPreflight = await preflight.runReport(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            plan: nil
        )
    }

    private func saveAndLaunchSampleRun() async {
        persistConfiguration()
        await providerRegistry.refreshHealth()
        await refreshPreflight()

        let launcher = SampleRunLauncher(
            modelContext: modelContext,
            executionService: executionService,
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )

        do {
            let run = try await launcher.launchSampleRun()
            wizardMessage = "Sample run started: \(run.workflowTitle)"
            isPresented = false
        } catch {
            wizardMessage = error.localizedDescription
        }
    }

    private func exportSettings() {
        persistConfiguration()
        let transfer = SettingsTransferService(
            appConfigurationStore: appConfigurationStore,
            providerSettingsStore: providerSettingsStore,
            secretStore: providerRegistry.secretStore
        )
        do {
            let url = try transfer.exportSettings()
            transferMessage = "Exported settings to \(url.lastPathComponent)"
        } catch {
            transferMessage = error.localizedDescription
        }
    }

    private func importLatestSettings() {
        let transfer = SettingsTransferService(
            appConfigurationStore: appConfigurationStore,
            providerSettingsStore: providerSettingsStore,
            secretStore: providerRegistry.secretStore
        )

        let importRoot = supportBundleExportPath.isEmpty
            ? (appConfigurationStore.configuration.supportBundleExportPath ?? AppConfiguration.defaultSupportRoot()
                .appendingPathComponent("exports", isDirectory: true).path)
            : supportBundleExportPath
        let fileURL = URL(fileURLWithPath: importRoot, isDirectory: true)
            .appendingPathComponent("chainworks-settings.json")

        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            transferMessage = "No exported settings found at \(fileURL.path)"
            return
        }

        do {
            _ = try transfer.importSettings(from: fileURL)
            runStorageBasePath = appConfigurationStore.configuration.runStorageBasePath
            worktreeBasePath = appConfigurationStore.configuration.worktreeBasePath ?? ""
            workflowSourcePath = appConfigurationStore.configuration.workflowSourcePath
            agentCatalogSourcePath = appConfigurationStore.configuration.agentCatalogSourcePath
            supportBundleExportPath = appConfigurationStore.configuration.supportBundleExportPath ?? ""
            transferMessage = "Imported settings from \(fileURL.lastPathComponent)"
            Task {
                await providerRegistry.refreshHealth()
                await refreshPreflight()
            }
        } catch {
            transferMessage = error.localizedDescription
        }
    }
}
