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
        case readiness = "System Readiness"
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
                    P031OperatorPlaceholder(
                        title: "Provider Settings",
                        message: "Provider configuration is owned by the control plane and packaged daemon.",
                        identifier: "provider-settings-view",
                        titleIdentifier: "provider-settings-title"
                    )
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
            Section("Overall Readiness") {
                HStack {
                    Text("Status")
                    Spacer()
                    if workbench.freshnessAndHealth?.isReadinessDeferred == true {
                        Label("Readiness Pending", systemImage: "clock.badge.questionmark")
                            .foregroundStyle(.secondary)
                    } else if workbench.freshnessAndHealth?.isSystemReady == true {
                        Label("Ready", systemImage: "checkmark.circle.fill")
                            .foregroundStyle(.green)
                    } else {
                        Label("Check Diagnostics", systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                    }
                }
            }
            
            Section {
                DisclosureGroup("Diagnostics & Configuration Paths") {
                    VStack(alignment: .leading, spacing: 10) {
                        LabeledContent("App Bundle", value: redactedPath(Bundle.main.bundlePath))
                        LabeledContent("Working Dir", value: redactedPath(FileManager.default.currentDirectoryPath))
                    }
                    .padding(.vertical, 4)
                }
            }
            .accessibilityIdentifier("settings-diagnostics-section")
            
            Section("Provider Health") {
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
                LabeledContent("Server Status", value: workbench.freshnessAndHealth?.daemonHealth ?? "Unknown")
                LabeledContent("MCP Hub", value: workbench.freshnessAndHealth?.mcpHubStatus ?? "Unknown")
                LabeledContent("Capabilities", value: workbench.freshnessAndHealth?.capabilitiesStatus ?? "Pending")
            }
            
            Section("Scheduler Health") {
                if let schedulerLabel = workbench.freshnessAndHealth?.schedulerHealth {
                    LabeledContent("Status", value: schedulerLabel)
                } else {
                    Text("Waiting for scheduler readback...")
                        .foregroundStyle(.secondary)
                }
            }
            
            Section("Daemon Connection") {
                if let daemon = runsModel.daemonLifecycle {
                    LabeledContent("Mode", value: daemon.state?.rawValue.capitalized ?? "Unknown")
                    LabeledContent("Uptime", value: "\(daemon.uptimeSeconds ?? 0)s")
                    // M4: PID is diagnostic-only and should not be surfaced by default
                    if showDiagnosticsDetail {
                        LabeledContent("PID", value: "\(daemon.pid ?? 0)")
                    }
                } else {
                    Text("Daemon disconnected or unavailable")
                        .foregroundStyle(.red)
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
                                Text("Return to Run")
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
                Section("Actionable Runs") {
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
                Toggle("Show Diagnostics Detail", isOn: $showDiagnosticsDetail)
                    .accessibilityIdentifier("settings-diagnostics-detail-toggle")
                Button("Refresh Readiness") {
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
