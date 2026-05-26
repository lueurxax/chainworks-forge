import SwiftUI

struct SettingsView: View {
    @ObservedObject var runsModel: P031ThinReadDashboardModel
    @ObservedObject var workbench: RunsWorkbenchPresentationModel
    // P036: return target from blocked/failed run that opened System Readiness.
    var returnRunID: String? = nil
    var onClearReturnRunID: (() -> Void)? = nil
    @State private var selectedSegment: Segment = .readiness
    @State private var showDiagnosticsDetail = false
    
    enum Segment: String, CaseIterable {
        case readiness = "System readiness"
        case provider = "Provider"
    }
    
    var body: some View {
        VStack(spacing: 0) {
            Picker("Segment", selection: $selectedSegment) {
                ForEach(Segment.allCases, id: \.self) { segment in
                    Text(segment.rawValue).tag(segment)
                }
            }
            .pickerStyle(.segmented)
            .padding()
            
            Divider()
            
            Group {
                switch selectedSegment {
                case .readiness:
                    systemReadinessView
                case .provider:
                    providerSettingsView
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityIdentifier("settings-view")
        .onReceive(NotificationCenter.default.publisher(for: .chainworksOpenSystemReadiness)) { _ in
            selectedSegment = .readiness
        }
    }
    
    private var systemReadinessView: some View {
        List {
            Section("Overall readiness") {
                HStack {
                    Text("Status")
                    Spacer()
                    if workbench.freshnessAndHealth?.isReadinessDeferred == true {
                        Label("Readiness pending", systemImage: "clock.badge.questionmark")
                            .foregroundStyle(.secondary)
                    } else if workbench.freshnessAndHealth?.isSystemReady == true {
                        Label("Ready", systemImage: "checkmark.circle.fill")
                            .foregroundStyle(ForgeStatusColor.success)
                    } else {
                        Label("Check diagnostics", systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(ForgeStatusColor.warning)
                    }
                }
            }
            
            Section {
                DisclosureGroup("Diagnostics and configuration paths") {
                    VStack(alignment: .leading, spacing: 10) {
                        LabeledContent("App bundle", value: redactedPath(Bundle.main.bundlePath))
                        LabeledContent("Working directory", value: redactedPath(FileManager.default.currentDirectoryPath))
                    }
                    .padding(.vertical, 4)
                }
            }
            .accessibilityIdentifier("settings-diagnostics-section")
            
            Section("Provider health") {
                if let readback = runsModel.schedulerHealth, !readback.activeProviders.isEmpty {
                    ForEach(readback.activeProviders) { provider in
                        LabeledContent(provider.providerFamily, value: "\(provider.activeCount) active")
                    }
                } else {
                    Text("No providers reported")
                        .foregroundStyle(.secondary)
                }
            }
            
            Section("Readiness") {
                LabeledContent("Server status", value: workbench.freshnessAndHealth?.daemonHealth ?? "Unknown")
                LabeledContent("MCP hub", value: workbench.freshnessAndHealth?.mcpHubStatus ?? "Unknown")
                LabeledContent("Capabilities", value: workbench.freshnessAndHealth?.capabilitiesStatus ?? "Pending")
            }
            
            Section("Scheduler health") {
                if let schedulerLabel = workbench.freshnessAndHealth?.schedulerHealth {
                    LabeledContent("Status", value: schedulerLabel)
                } else {
                    Text("Waiting for scheduler readback...")
                        .foregroundStyle(.secondary)
                }
            }
            
            Section("Daemon connection") {
                if let daemon = runsModel.daemonLifecycle {
                    LabeledContent("Mode", value: daemon.state?.rawValue.capitalized ?? "Unknown")
                    LabeledContent("Uptime", value: "\(daemon.uptimeSeconds ?? 0)s")
                    // M4: PID is diagnostic-only and should not be surfaced by default
                    if showDiagnosticsDetail {
                        LabeledContent("PID", value: "\(daemon.pid ?? 0)")
                    }
                } else {
                    Text("Daemon disconnected or unavailable")
                        .foregroundStyle(ForgeStatusColor.error)
                }
            }

            // P036: return target — shown when the operator navigated here from a specific run.
            if let runID = returnRunID {
                let returnTitle = runsModel.runsHome?.rows.first(where: { $0.runID == runID })?.title
                Section("Context") {
                    Button {
                        NotificationCenter.default.post(
                            name: .chainworksOpenRunInRunsHome,
                            object: runID
                        )
                        onClearReturnRunID?()
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Return to run")
                                    .font(.body)
                                if let title = returnTitle {
                                    Text(title)
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Spacer()
                            Image(systemName: "arrow.uturn.left")
                                .foregroundStyle(Color.accentColor)
                        }
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("settings-return-to-run")
                    .accessibilityLabel(returnTitle.map { "Return to run: \($0)" } ?? "Return to run")
                }
            }

            if let blocked = runsModel.runsHome?.rows.filter({ $0.lane == .blocked }), !blocked.isEmpty {
                Section("Actionable runs") {
                    ForEach(blocked, id: \.runID) { run in
                        Button {
                            NotificationCenter.default.post(name: .chainworksOpenRunInRunsHome, object: run.runID)
                        } label: {
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(run.title)
                                    Text(run.runID).font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer()
                                Image(systemName: "arrow.right.circle")
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            
            Section {
                Toggle("Show diagnostics detail", isOn: $showDiagnosticsDetail)
                    .accessibilityIdentifier("settings-diagnostics-detail-toggle")
                Button("Refresh readiness") {
                    Task {
                        await runsModel.refreshDaemonLifecycle()
                    }
                }
                .frame(maxWidth: .infinity)
            }
        }
        .listStyle(.inset)
        .accessibilityIdentifier("system-readiness-view")
    }

    private var providerSettingsView: some View {
        List {
            Section("Provider capacity") {
                if let readback = runsModel.schedulerHealth, !readback.activeProviders.isEmpty {
                    ForEach(readback.activeProviders) { provider in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(provider.providerFamily)
                                    .font(ForgeTypography.cardTitle)
                                Spacer()
                                StatusCapsule(
                                    text: "\(provider.activeCount) active",
                                    color: provider.activeCount > 0 ? ForgeStatusColor.running : ForgeStatusColor.neutral,
                                    icon: provider.activeCount > 0 ? "bolt.fill" : "pause.circle",
                                    size: .small
                                )
                            }
                            Text("Control-plane provider family")
                                .font(ForgeTypography.supporting)
                                .foregroundStyle(ForgeColor.Text.secondary)
                        }
                        .padding(.vertical, 4)
                    }
                } else {
                    ContentUnavailableView(
                        "No provider readback",
                        systemImage: "externaldrive.badge.questionmark",
                        description: Text("Refresh readiness to load provider capacity from the control plane.")
                    )
                }
            }

            Section("Runtime diagnostics") {
                LabeledContent("Daemon", value: workbench.freshnessAndHealth?.daemonHealth ?? "Unknown")
                LabeledContent("Scheduler", value: workbench.freshnessAndHealth?.schedulerHealth ?? "Unavailable")
                LabeledContent("Capabilities", value: workbench.freshnessAndHealth?.capabilitiesStatus ?? "Pending")
                Button("Refresh provider diagnostics") {
                    Task {
                        await runsModel.refreshDaemonLifecycle()
                    }
                }
            }
        }
        .listStyle(.inset)
        .accessibilityIdentifier("provider-settings-view")
    }
    
    private func redactedPath(_ path: String) -> String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        if path.hasPrefix(home) {
            return "~" + path.dropFirst(home.count)
        }
        // M1: mask non-$HOME absolute paths to avoid leaking workspace structure
        // (e.g. /Volumes/…, /var/…, /private/…)
        if path.hasPrefix("/") {
            return "<redacted>"
        }
        return path
    }
}
