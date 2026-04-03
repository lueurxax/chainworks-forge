import SwiftUI
import SwiftData

struct FirstRunSetupWizard: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry
    @Environment(GooseServerManager.self) private var gooseServerManager

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
    @State private var selectedAssistantProviderID: UUID?
    @State private var isLaunching = false
    private let showsUITestReadyMarker = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] != nil

    // Proposal 012 (H-03): Wizard steps
    enum WizardStep: Int, CaseIterable {
        case workspace = 0
        case providers = 1
        case verification = 2
        case launch = 3

        var title: String {
            switch self {
            case .workspace: return "Workspace"
            case .providers: return "Providers"
            case .verification: return "Verification"
            case .launch: return "Launch"
            }
        }

        var icon: String {
            switch self {
            case .workspace: return "folder"
            case .providers: return "server.rack"
            case .verification: return "checkmark.shield"
            case .launch: return "play.circle"
            }
        }
    }

    @State private var currentStep: WizardStep = .workspace

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Proposal 012 (H-03): Step indicator strip
                wizardStepIndicator
                    .padding(.vertical, DesignTokens.Spacing.medium)
                    .padding(.horizontal)
                    .background(.bar)

                Divider()

                Form {
                if showsUITestReadyMarker {
                    Section {
                        Button("First Run Setup Ready") {}
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("first-run-setup-surface-ready")
                        Button("First Run Setup Root") {}
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("first-run-setup-wizard")
                    }
                }

                // Show sections based on current step (all visible but highlighted)
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
                    Button("Add Codex via Goose") {
                        providerSettingsStore.upsert(provider: gooseFirstProvider(
                            family: .codex,
                            displayName: "Codex Goose",
                            defaultModel: "gpt-5-codex"
                        ))
                    }
                    .accessibilityIdentifier("first-run-add-codex")
                    Button("Add Claude via Goose") {
                        providerSettingsStore.upsert(provider: gooseFirstProvider(
                            family: .claude,
                            displayName: "Claude Goose",
                            defaultModel: "sonnet"
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
                    Text("Codex and Claude are Goose-first in the app. Use Goose-backed transport unless you intentionally need CLI fallback.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)

                    ForEach(providerSettingsStore.settings.configuredProviders.filter { $0.family.gooseFirstPreferred }) { provider in
                        Button("Open \(provider.family.displayName) Goose Assistant") {
                            selectedAssistantProviderID = provider.id
                        }
                        .accessibilityIdentifier("first-run-open-assistant-\(provider.family.rawValue)")
                    }
                }

                Section("Managed Goose Server") {
                    LabeledContent("State", value: gooseServerManager.statusSummary)
                        .accessibilityIdentifier("first-run-goose-state")
                    if let baseURL = appConfigurationStore.configuration.gooseServerBaseURL {
                        LabeledContent("Base URL", value: baseURL.absoluteString)
                    }
                    Button("Start Managed Server") {
                        Task {
                            await gooseServerManager.ensureRunning()
                            await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
                            await refreshPreflight()
                        }
                    }
                    .accessibilityIdentifier("first-run-start-goose")
                }

                Section("Verification") {
                    Button("Refresh Provider Diagnostics") {
                        Task {
                            await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
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
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)

                    // Proposal 012 (L-12): Footer-level blocking progress during save/launch
                    Button {
                        Task { await saveAndLaunchSampleRun() }
                    } label: {
                        HStack {
                            if isLaunching {
                                ProgressView()
                                    .controlSize(.small)
                                Text("Launching…")
                            } else {
                                Label("Save and Launch Sample Run", systemImage: "play.circle.fill")
                            }
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(providerSettingsStore.settings.configuredProviders.isEmpty || isLaunching)
                    .accessibilityIdentifier("first-run-launch-sample-run")

                    if let wizardMessage {
                        Text(wizardMessage)
                            .font(DesignTokens.Typography.supporting)
                            .foregroundStyle(wizardMessage.contains("started") ? DesignTokens.Status.success : DesignTokens.Status.error)
                            .accessibilityIdentifier("first-run-message")
                    }
                }
                } // end Form
            } // end VStack
            .navigationTitle("First Run Setup")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { isPresented = false }
                        // Proposal 012 (L-09): Escape to dismiss wizard
                        .keyboardShortcut(.escape, modifiers: [])
                        .accessibilityIdentifier("first-run-close")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        persistConfiguration()
                        Task {
                            await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
                            await refreshPreflight()
                        }
                        isPresented = false
                    }
                    .buttonStyle(.borderedProminent)
                    // Proposal 012 (L-09): ⌘S to save wizard configuration
                    .keyboardShortcut("s", modifiers: [.command])
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
            .sheet(isPresented: Binding(
                get: { selectedAssistantProviderID != nil },
                set: { if !$0 { selectedAssistantProviderID = nil } }
            )) {
                if let selectedAssistantProviderID {
                    GooseProviderConnectionAssistantView(
                        providerID: selectedAssistantProviderID,
                        origin: .firstRunWizard
                    )
                        .environment(appConfigurationStore)
                        .environment(providerSettingsStore)
                        .environment(providerRegistry)
                        .environment(gooseServerManager)
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

    private func gooseFirstProvider(family: ProviderFamily, displayName: String, defaultModel: String) -> ConfiguredProvider {
        ConfiguredProvider(
            family: family,
            displayName: displayName,
            transport: .gooseServer,
            endpoint: appConfigurationStore.configuration.gooseServerBaseURL?.absoluteString,
            authMode: appConfigurationStore.configuration.gooseServerSecretKey == nil ? .none : .apiKey,
            defaultModel: defaultModel
        )
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

    // Proposal 012 (L-12): Save and launch with loading + error state
    private func saveAndLaunchSampleRun() async {
        isLaunching = true
        defer { isLaunching = false }
        wizardMessage = nil
        persistConfiguration()
        await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
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

    // Proposal 012 (H-03): Step indicator strip
    private var wizardStepIndicator: some View {
        HStack(spacing: 0) {
            ForEach(WizardStep.allCases, id: \.rawValue) { step in
                HStack(spacing: DesignTokens.Spacing.compact) {
                    Image(systemName: stepCompleted(step) ? "checkmark.circle.fill" : step.icon)
                        .font(.caption)
                        .foregroundStyle(stepColor(step))
                    Text(step.title)
                        .font(currentStep == step ? DesignTokens.Typography.cardTitle : DesignTokens.Typography.supporting)
                        .foregroundStyle(currentStep == step ? .primary : .secondary)
                }
                .padding(.horizontal, DesignTokens.Spacing.small)
                .padding(.vertical, DesignTokens.Spacing.compact)
                .background(currentStep == step ? Color.accentColor.opacity(0.1) : Color.clear, in: RoundedRectangle(cornerRadius: 6))
                .onTapGesture { currentStep = step }

                if step.rawValue < WizardStep.allCases.count - 1 {
                    Image(systemName: "chevron.right")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 2)
                }
            }
        }
    }

    private func stepCompleted(_ step: WizardStep) -> Bool {
        switch step {
        case .workspace:
            return !runStorageBasePath.isEmpty && !workflowSourcePath.isEmpty && !agentCatalogSourcePath.isEmpty
        case .providers:
            return !providerSettingsStore.settings.configuredProviders.isEmpty
        case .verification:
            return latestPreflight?.status == .pass || latestPreflight?.status == .warn
        case .launch:
            return wizardMessage?.contains("started") ?? false
        }
    }

    private func stepColor(_ step: WizardStep) -> Color {
        if stepCompleted(step) { return DesignTokens.Status.success }
        if currentStep == step { return DesignTokens.Action.primary }
        return .secondary
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
                await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
                await refreshPreflight()
            }
        } catch {
            transferMessage = error.localizedDescription
        }
    }
}

#Preview("First Run Setup — Seeded") {
    @Previewable @State var isPresented = true
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
    let gooseServerManager = GooseServerManager(appConfigurationStore: appConfigurationStore)

    return FirstRunSetupWizard(isPresented: $isPresented)
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .environment(gooseServerManager)
        .frame(width: 760, height: 860)
}
