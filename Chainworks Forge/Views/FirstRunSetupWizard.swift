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
    @State private var workflowSourcePath = ""
    @State private var agentCatalogSourcePath = ""
    @State private var wizardMessage: String?
    @State private var latestPreflight: PreflightReport?

    var body: some View {
        NavigationStack {
            Form {
                Section("Workspace & YAML") {
                    TextField("Run Storage Base Path", text: $runStorageBasePath)
                    TextField("Workflow Source Path", text: $workflowSourcePath)
                    TextField("Agent Catalog Source Path", text: $agentCatalogSourcePath)
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
                    Button("Add Claude CLI") {
                        providerSettingsStore.upsert(provider: ConfiguredProvider(
                            family: .claude,
                            displayName: "Claude CLI",
                            transport: .cli,
                            authMode: .none,
                            defaultModel: "claude-sonnet-4"
                        ))
                    }
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

                    Text("\(providerSettingsStore.settings.configuredProviders.count) provider(s) configured")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Section("Verification") {
                    Button("Refresh Provider Health") {
                        Task {
                            await providerRegistry.refreshHealth()
                            await refreshPreflight()
                        }
                    }

                    if let latestPreflight {
                        LabeledContent("Preflight", value: latestPreflight.status.rawValue.capitalized)
                        LabeledContent("Configuration Source", value: latestPreflight.configurationSource.displayName)
                        if let issue = latestPreflight.blockingIssues.first {
                            Text(issue)
                                .font(.caption)
                                .foregroundStyle(.red)
                        } else if let warning = latestPreflight.warnings.first {
                            Text(warning)
                                .font(.caption)
                                .foregroundStyle(.orange)
                        } else {
                            Text("Configuration is ready for a sample run.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    } else {
                        Text("Save configuration, then verify provider and workspace readiness.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
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

                    if let wizardMessage {
                        Text(wizardMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .navigationTitle("First Run Setup")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { isPresented = false }
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
                }
            }
            .task {
                runStorageBasePath = appConfigurationStore.configuration.runStorageBasePath
                workflowSourcePath = appConfigurationStore.configuration.workflowSourcePath
                agentCatalogSourcePath = appConfigurationStore.configuration.agentCatalogSourcePath
                await refreshPreflight()
            }
        }
        .frame(minWidth: 520, minHeight: 420)
        .accessibilityIdentifier("first-run-setup-wizard")
    }

    private func persistConfiguration() {
        appConfigurationStore.update {
            $0.runStorageBasePath = runStorageBasePath
            $0.workflowSourcePath = workflowSourcePath
            $0.agentCatalogSourcePath = agentCatalogSourcePath
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
}
