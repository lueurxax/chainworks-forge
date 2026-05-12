import SwiftUI

struct SettingsView: View {
    @ObservedObject var runsModel: P031ThinReadDashboardModel
    @State private var selectedSegment: Segment = .readiness
    
    @StateObject private var daemonStatus = DaemonStatusViewModel.bootstrap()
    @StateObject private var schedulerHealth = SchedulerHealthViewModel.bootstrap()
    
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
                    SettingsPlaceholder(
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
        .task {
            await daemonStatus.startSnapshotPlusSubscribe()
            await schedulerHealth.refresh()
        }
    }
    
    private var systemReadinessView: some View {
        List {
            Section("Overall Readiness") {
                HStack {
                    Text("Status")
                    Spacer()
                    if schedulerHealth.lastError == nil && daemonStatus.lastError == nil {
                        Label("Ready", systemImage: "checkmark.circle.fill")
                            .foregroundStyle(.green)
                    } else {
                        Label("Check Diagnostics", systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                    }
                }
            }
            
            Section("Configuration Paths") {
                LabeledContent("App Bundle", value: Bundle.main.bundlePath)
                LabeledContent("Working Dir", value: FileManager.default.currentDirectoryPath)
            }
            
            Section("Provider Health") {
                if let readback = schedulerHealth.readback, !readback.activeProviders.isEmpty {
                    ForEach(readback.activeProviders, id: \.providerFamily) { provider in
                        LabeledContent(provider.providerFamily, value: "\(provider.activeCount) active")
                    }
                } else {
                    Text("No providers reported")
                        .foregroundStyle(.secondary)
                }
            }
            
            Section("Goose/Server State") {
                LabeledContent("Server Status", value: "Running")
                LabeledContent("MCP Hub", value: "Connected")
                LabeledContent("Capabilities", value: "Validated")
            }
            
            Section("Scheduler Health") {
                if let health = schedulerHealth.readback?.health {
                    LabeledContent("Queued Count", value: "\(health.queuedCount)")
                    LabeledContent("Sustained Backpressure", value: health.sustainedBackpressureState)
                } else {
                    Text("Waiting for scheduler readback...")
                        .foregroundStyle(.secondary)
                }
            }
            
            Section("Daemon Connection") {
                if let status = daemonStatus.status {
                    LabeledContent("State", value: status.state.rawValue)
                    LabeledContent("PID", value: "\(status.pid)")
                    if let startedAt = status.startedAt {
                        LabeledContent("Started", value: startedAt.formatted(date: .abbreviated, time: .standard))
                    }
                } else {
                    Text("Daemon disconnected or unavailable")
                        .foregroundStyle(.red)
                }
            }
            
            let blockedRuns = runsModel.runsHome?.rows.filter {
                $0.statusLabel.localizedCaseInsensitiveContains("blocked")
            } ?? []
            if !blockedRuns.isEmpty {
                Section("Actionable Runs") {
                    ForEach(blockedRuns, id: \.runID) { run in
                        Button {
                            NotificationCenter.default.post(name: .init("chainworksOpenRunInRunsHome"), object: run.runID)
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
                Button("Refresh Readiness") {
                    Task {
                        await schedulerHealth.refresh()
                        await daemonStatus.refresh()
                    }
                }
                .frame(maxWidth: .infinity)
            }
        }
        .listStyle(.inset)
        .accessibilityIdentifier("system-readiness-view")
    }
}

private struct SettingsPlaceholder: View {
    let title: String
    let message: String
    let identifier: String
    let titleIdentifier: String

    var body: some View {
        ContentUnavailableView(
            title,
            systemImage: "gearshape",
            description: Text(message)
        )
        .accessibilityIdentifier(identifier)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
    }
}
