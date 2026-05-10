import SwiftUI

struct SettingsView: View {
    @State private var selectedSegment: Segment = .readiness
    
    @StateObject private var daemonStatus = DaemonStatusViewModel.bootstrap()
    @StateObject private var schedulerHealth = SchedulerHealthViewModel.bootstrap()
    
    enum Segment: String, CaseIterable {
        case readiness = "Readiness"
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
                    LabeledContent("Mode", value: status.mode)
                    LabeledContent("Uptime", value: "\(status.uptimeSeconds)s")
                } else {
                    Text("Daemon disconnected or unavailable")
                        .foregroundStyle(.red)
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
        .listStyle(.insetGrouped)
        .accessibilityIdentifier("system-readiness-view")
    }
}
