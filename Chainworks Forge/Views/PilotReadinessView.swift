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
    @State private var showPreflightReport = false

    @State private var isRefreshing = false
    private let showsUITestReadyMarker = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_DIRECT_SURFACE"] != nil

    var body: some View {
        NavigationStack {
            List {
                if showsUITestReadyMarker {
                    Section {
                        Button("Pilot Readiness Ready") {}
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("pilot-readiness-surface-ready")
                    }
                }

                // Proposal 012 (H-02): Hero status banner
                Section {
                    readinessHeroBanner
                }

                // Proposal 012 (H-02): Configuration paths in collapsible DisclosureGroup
                Section {
                    DisclosureGroup("Configuration Paths") {
                        LabeledContent("Source", value: appConfigurationStore.configuration.activeConfigurationSource.displayName)
                        LabeledContent("Workflow", value: appConfigurationStore.configuration.workflowSourcePath)
                        LabeledContent("Catalog", value: appConfigurationStore.configuration.agentCatalogSourcePath)
                        LabeledContent("Run Storage", value: appConfigurationStore.configuration.runStorageBasePath)
                        if let worktreeBasePath = appConfigurationStore.configuration.worktreeBasePath {
                            LabeledContent("Worktree Base", value: worktreeBasePath)
                        }
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
                                if let blockingIssues = providerRegistry.healthSnapshot(for: provider.id)?.blockingIssues,
                                   !blockingIssues.isEmpty {
                                    ForEach(blockingIssues, id: \.self) { issue in
                                        Text(issue)
                                            .font(.caption2)
                                            .foregroundStyle(.red)
                                    }
                                }
                                if let checkedAt = providerRegistry.healthSnapshot(for: provider.id)?.checkedAt {
                                    Text("Checked \(checkedAt.formatted(date: .omitted, time: .shortened))")
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                }
                                if let report = providerRegistry.troubleshootingReport(for: provider.id) {
                                    ProviderTroubleshootingPanel(report: report)
                                }
                            }
                        }
                    }
                }

                Section("Diagnostics") {
                    if let readinessReport {
                        LabeledContent("Preflight", value: readinessReport.status.rawValue.capitalized)
                            .accessibilityIdentifier("pilot-readiness-preflight-status")
                        LabeledContent("Source", value: readinessReport.configurationSource.displayName)
                            .accessibilityIdentifier("pilot-readiness-source")

                        ForEach(readinessChecks(category: "Providers")) { check in
                            readinessCheckRow(check)
                        }

                        ForEach(readinessChecks(category: "Catalog")) { check in
                            readinessCheckRow(check)
                        }

                        if !readinessChecks(category: "Skills").isEmpty {
                            Color.clear
                                .frame(width: 1, height: 1)
                                .accessibilityIdentifier("pilot-readiness-skills-section")
                        }
                        ForEach(readinessChecks(category: "Skills")) { check in
                            readinessCheckRow(check)
                        }

                        ForEach(readinessChecks(category: "Workspace")) { check in
                            readinessCheckRow(check)
                        }

                        ForEach(readinessChecks(category: "Permissions")) { check in
                            readinessCheckRow(check)
                        }

                        ForEach(readinessChecks(category: "Environment")) { check in
                            readinessCheckRow(check)
                        }

                        if !readinessReport.warnings.isEmpty {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Warnings")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                ForEach(readinessReport.warnings, id: \.self) { warning in
                                    Text(warning)
                                        .font(.caption)
                                        .foregroundStyle(.orange)
                                }
                            }
                        }

                        if !readinessReport.blockingIssues.isEmpty {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Blocking Issues")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                ForEach(readinessReport.blockingIssues, id: \.self) { issue in
                                    Text(issue)
                                        .font(.caption)
                                        .foregroundStyle(.red)
                                }
                            }
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
                    .accessibilityIdentifier("pilot-readiness-open-wizard")

                    Button("Launch Sample Run Path") {
                        Task { await launchSampleRun() }
                    }
                    .disabled(providerSettingsStore.settings.configuredProviders.isEmpty)
                    .accessibilityIdentifier("pilot-readiness-launch-sample-run")

                    if let sampleRunMessage {
                        Text(sampleRunMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("pilot-readiness-sample-run-message")
                    }
                }

                Section("Support") {
                    Button("Refresh Readiness") {
                        Task { await refreshReadiness() }
                    }
                    .accessibilityIdentifier("pilot-readiness-refresh")
                    if readinessReport != nil {
                        Button("View Preflight Report") {
                            showPreflightReport = true
                        }
                        .accessibilityIdentifier("pilot-readiness-view-preflight-report")
                    }
                    Button("Export Support Bundle") {
                        Task { await exportSupportBundle() }
                    }
                    .accessibilityIdentifier("pilot-readiness-export-support")
                    if let exportMessage {
                        Text(exportMessage)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("pilot-readiness-export-message")
                    }
                }
            }
            .navigationTitle("Pilot Readiness")
            .accessibilityIdentifier("pilot-readiness-view")
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    Button("Refresh Readiness") {
                        Task { await refreshReadiness() }
                    }
                    .accessibilityIdentifier("pilot-readiness-toolbar-refresh")
                    Button("Open First Run Wizard") {
                        showWizard = true
                    }
                    .accessibilityIdentifier("pilot-readiness-open-wizard")
                }
            }
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
            .sheet(isPresented: $showPreflightReport) {
                if let readinessReport {
                    NavigationStack {
                        PreflightReportView(report: readinessReport)
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

    // Proposal 012 (L-12): Refresh with loading state
    private func refreshReadiness() async {
        isRefreshing = true
        defer { isRefreshing = false }
        await providerRegistry.refreshDiagnostics(appConfiguration: appConfigurationStore.configuration)
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
            return DesignTokens.Status.success
        case .warn:
            return DesignTokens.Status.warning
        case .fail:
            return DesignTokens.Status.error
        }
    }

    // MARK: - Proposal 012 (H-02): Hero Readiness Banner

    @ViewBuilder
    private var readinessHeroBanner: some View {
        if isRefreshing {
            HStack(spacing: DesignTokens.Spacing.medium) {
                ProgressView()
                    .controlSize(.regular)
                VStack(alignment: .leading) {
                    Text("Checking Readiness…")
                        .font(.title3.bold())
                    Text("Running diagnostics and preflight checks.")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.vertical, DesignTokens.Spacing.small)
        } else if let report = readinessReport {
            let passCount = report.checks.filter { $0.status == .pass }.count
            let totalCount = report.checks.count
            let hasBlockers = !report.blockingIssues.isEmpty
            let hasWarnings = !report.warnings.isEmpty

            HStack(spacing: DesignTokens.Spacing.medium) {
                Image(systemName: hasBlockers ? "xmark.circle.fill" : hasWarnings ? "exclamationmark.triangle.fill" : "checkmark.circle.fill")
                    .font(.system(size: 36))
                    .foregroundStyle(hasBlockers ? DesignTokens.Status.error : hasWarnings ? DesignTokens.Status.warning : DesignTokens.Status.success)
                    .symbolRenderingMode(.multicolor)

                VStack(alignment: .leading, spacing: DesignTokens.Spacing.compact) {
                    Text(hasBlockers ? "\(report.blockingIssues.count) Issue(s) Found" : hasWarnings ? "Ready with Warnings" : "System Ready")
                        .font(.title3.bold())
                    Text("\(passCount)/\(totalCount) checks pass")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                // Progress ring
                ZStack {
                    Circle()
                        .stroke(Color.secondary.opacity(0.2), lineWidth: 4)
                    Circle()
                        .trim(from: 0, to: totalCount > 0 ? CGFloat(passCount) / CGFloat(totalCount) : 0)
                        .stroke(
                            hasBlockers ? DesignTokens.Status.error : hasWarnings ? DesignTokens.Status.warning : DesignTokens.Status.success,
                            style: StrokeStyle(lineWidth: 4, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                    Text("\(passCount)/\(totalCount)")
                        .font(DesignTokens.Typography.micro.bold())
                }
                .frame(width: 44, height: 44)
            }
            .padding(.vertical, DesignTokens.Spacing.small)
            .accessibilityIdentifier("pilot-readiness-title")
        } else {
            HStack(spacing: DesignTokens.Spacing.medium) {
                Image(systemName: "questionmark.circle")
                    .font(.system(size: 36))
                    .foregroundStyle(.secondary)
                VStack(alignment: .leading) {
                    Text("Pilot Readiness")
                        .font(.title3.bold())
                        .accessibilityIdentifier("pilot-readiness-title")
                    Text("Refresh to check system readiness.")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.vertical, DesignTokens.Spacing.small)
        }
    }
}

#Preview("Pilot Readiness — Seeded") {
    let container = PreviewSupport.makeModelContainer(seed: PreviewSupport.seedOperatorData)
    let appConfigurationStore = PreviewSupport.makeAppConfigurationStore()
    let providerSettingsStore = PreviewSupport.makeProviderSettingsStore()
    let providerRegistry = PreviewSupport.makeProviderRegistry(settingsStore: providerSettingsStore)
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)

    return PilotReadinessView()
        .modelContainer(container)
        .environment(executionService)
        .environment(appConfigurationStore)
        .environment(providerSettingsStore)
        .environment(providerRegistry)
        .frame(width: 1100, height: 820)
}
