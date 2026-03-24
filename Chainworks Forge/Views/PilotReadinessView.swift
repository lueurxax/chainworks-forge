import SwiftUI
import SwiftData

struct PilotReadinessView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService
    @Environment(AppConfigurationStore.self) private var appConfigurationStore
    @Environment(ProviderSettingsStore.self) private var providerSettingsStore
    @Environment(ProviderRegistry.self) private var providerRegistry

    @Query(sort: \Run.startedAt, order: .reverse) private var runs: [Run]
    @State private var exportMessage: String?
    @State private var sampleRunMessage: String?
    @State private var readinessReport: PreflightReport?
    @State private var showWizard = false

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Text("Pilot Readiness")
                        .font(.title3.bold())
                        .accessibilityIdentifier("pilot-readiness-title")
                }
                Section("Configuration") {
                    LabeledContent("Source", value: appConfigurationStore.configuration.activeConfigurationSource.displayName)
                    LabeledContent("Workflow", value: appConfigurationStore.configuration.workflowSourcePath)
                    LabeledContent("Catalog", value: appConfigurationStore.configuration.agentCatalogSourcePath)
                    LabeledContent("Run Storage", value: appConfigurationStore.configuration.runStorageBasePath)
                    if let worktreeBasePath = appConfigurationStore.configuration.worktreeBasePath {
                        LabeledContent("Worktree Base", value: worktreeBasePath)
                    }
                }

                Section("Providers") {
                    if providerRegistry.configuredProviders.isEmpty {
                        Text("No providers configured")
                            .foregroundStyle(.secondary)
                    } else {
                        if let lastRefreshedAt = providerRegistry.lastRefreshedAt {
                            LabeledContent("Health Refreshed", value: lastRefreshedAt.formatted(date: .omitted, time: .standard))
                        } else {
                            LabeledContent("Health Refreshed", value: "Stale until refresh")
                        }

                        ForEach(providerRegistry.configuredProviders) { provider in
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Text(provider.displayName)
                                    Spacer()
                                    Text(provider.family.displayName)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                Text(providerRegistry.healthSnapshot(for: provider.id)?.summary ?? "Health not refreshed")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                if let checkedAt = providerRegistry.healthSnapshot(for: provider.id)?.checkedAt {
                                    Text("Checked \(checkedAt.formatted(date: .omitted, time: .shortened))")
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                }
                            }
                        }
                    }
                }

                Section("Diagnostics") {
                    if let readinessReport {
                        LabeledContent("Preflight", value: readinessReport.status.rawValue.capitalized)
                        LabeledContent("Source", value: readinessReport.configurationSource.displayName)

                        ForEach(readinessChecks(category: "Catalog")) { check in
                            readinessCheckRow(check)
                        }

                        ForEach(readinessChecks(category: "Workspace")) { check in
                            readinessCheckRow(check)
                        }
                    } else {
                        Text("Readiness checks run after refresh.")
                            .foregroundStyle(.secondary)
                    }
                }

                Section("Operator Status") {
                    LabeledContent("Pending Approvals", value: "\(executionService.pendingApprovalCount)")
                    LabeledContent("Blocked Runs", value: "\(executionService.blockedRunCount)")
                    LabeledContent("Failed Runs", value: "\(executionService.failedRunCount)")
                    if let latestCompletedRun = runs.first(where: { $0.status == .completed }) {
                        LabeledContent("Last Successful Run", value: latestCompletedRun.workflowTitle)
                    }
                }

                Section("Pilot Actions") {
                    Button("Open First Run Wizard") {
                        showWizard = true
                    }

                    Button("Launch Sample Run Path") {
                        Task { await launchSampleRun() }
                    }
                    .disabled(providerSettingsStore.settings.configuredProviders.isEmpty)

                    if let sampleRunMessage {
                        Text(sampleRunMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                Section("Support") {
                    Button("Refresh Readiness") {
                        Task { await refreshReadiness() }
                    }
                    Button("Export Support Bundle") {
                        Task { await exportSupportBundle() }
                    }
                    if let exportMessage {
                        Text(exportMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .navigationTitle("Pilot Readiness")
            .accessibilityIdentifier("pilot-readiness-view")
            .task {
                await refreshReadiness()
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

    private func exportSupportBundle() async {
        let exporter = SupportBundleExporter(
            modelContext: modelContext,
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )
        do {
            exportMessage = try await exporter.exportBundle().path
        } catch {
            exportMessage = error.localizedDescription
        }
    }

    private func refreshReadiness() async {
        await providerRegistry.refreshHealth()
        let preflight = PreflightService(
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )
        readinessReport = await preflight.runReport(
            workflowURL: appConfigurationStore.configuration.workflowSourceURL,
            catalogURL: appConfigurationStore.configuration.agentCatalogSourceURL,
            plan: nil
        )
    }

    private func launchSampleRun() async {
        let launcher = SampleRunLauncher(
            modelContext: modelContext,
            executionService: executionService,
            appConfigurationStore: appConfigurationStore,
            providerRegistry: providerRegistry
        )

        do {
            let run = try await launcher.launchSampleRun()
            sampleRunMessage = "Sample run started: \(run.workflowTitle)"
        } catch {
            sampleRunMessage = error.localizedDescription
        }
    }

    private func readinessChecks(category: String) -> [PreflightCheck] {
        readinessReport?.checks.filter { $0.category == category } ?? []
    }

    @ViewBuilder
    private func readinessCheckRow(_ check: PreflightCheck) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(check.title)
                Spacer()
                Text(check.status.rawValue.uppercased())
                    .font(.caption2.bold())
                    .foregroundStyle(statusColor(check.status))
            }
            Text(check.message)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 2)
    }

    private func statusColor(_ status: PreflightCheckStatus) -> Color {
        switch status {
        case .pass:
            return .green
        case .warn:
            return .orange
        case .fail:
            return .red
        }
    }
}
