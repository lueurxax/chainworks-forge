import SwiftUI
import SwiftData

struct AgentSessionInspector: View {
    let lineage: AgentSessionLineage
    @Environment(\.modelContext) private var modelContext
    
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            headerSection
            
            if let activeGen = activeGeneration {
                activeGenerationSection(activeGen)
            } else {
                ContentUnavailableView("No Active Session", systemImage: "bolt.slash.fill", description: Text("The session lineage is currently inactive or was reset."))
            }
            
            Divider()
            
            eventHistorySection
        }
        .padding()
    }
    
    private var activeGeneration: AgentSessionGeneration? {
        guard let activeID = lineage.activeGenerationID else { return nil }
        return lineage.generations.first(where: { $0.id == activeID })
    }
    
    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Agent Session Lineage")
                .font(.headline)
            HStack {
                Text("Agent ID:")
                    .foregroundStyle(.secondary)
                Text(lineage.agentID)
                    .monospaced()
            }
            .font(.caption)
            
            HStack {
                Text("Reuse Scope:")
                    .foregroundStyle(.secondary)
                Text(lineage.sessionReuseScope.rawValue)
            }
            .font(.caption)
        }
    }
    
    private func activeGenerationSection(_ gen: AgentSessionGeneration) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Label("Active Generation #\(gen.generation)", systemImage: "cpu.fill")
                        .font(.subheadline.bold())
                    Spacer()
                    StatusCapsule(text: gen.status.rawValue.capitalized, color: statusColor(gen.status), size: .small)
                }
                
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                    GridRow {
                        Text("Provider Session ID")
                            .foregroundStyle(.secondary)
                        Text(gen.providerSessionID ?? "None")
                            .monospaced()
                    }
                    GridRow {
                        Text("Turns")
                            .foregroundStyle(.secondary)
                        Text("\(gen.turnCount)")
                    }
                    GridRow {
                        Text("Input Tokens (Est.)")
                            .foregroundStyle(.secondary)
                        Text("\(gen.estimatedInputTokens)")
                    }
                    GridRow {
                        Text("Cumulative Cost")
                            .foregroundStyle(.secondary)
                        Text("\(gen.cumulativeCostCents)c")
                    }
                }
                .font(.caption)
                
                if let fingerprint = String(gen.bindingFingerprint.prefix(8)).appending("...") as String? {
                    HStack {
                        Text("Fingerprint:")
                            .foregroundStyle(.secondary)
                        Text(fingerprint)
                            .monospaced()
                    }
                    .font(.caption2)
                }
            }
        } label: {
            Label("Current Status", systemImage: "info.circle")
        }
    }
    
    private var eventHistorySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Session Events", systemImage: "clock.arrow.circlepath")
                .font(.subheadline.bold())
            
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    let sortedEvents = lineage.events.sorted(by: { $0.recordedAt > $1.recordedAt })
                    if sortedEvents.isEmpty {
                        Text("No events recorded yet.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(sortedEvents) { event in
                            HStack(alignment: .top, spacing: 12) {
                                Image(systemName: eventIcon(event.eventType))
                                    .foregroundStyle(eventColor(event.eventType))
                                    .font(.caption)
                                    .frame(width: 16)
                                
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(event.eventType.rawValue.replacingOccurrences(of: "_", with: " ").capitalized)
                                        .font(.caption.bold())
                                    Text(event.recordedAt.formatted(.dateTime.hour().minute().second()))
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                            }
                            .padding(.vertical, 4)
                        }
                    }
                }
            }
        }
    }
    
    private func statusColor(_ status: AgentSessionStatus) -> Color {
        switch status {
        case .active: return .green
        case .invalidated: return .orange
        case .closed: return .gray
        case .reset: return .red
        }
    }
    
    private func eventIcon(_ type: AgentSessionEventType) -> String {
        switch type {
        case .created: return "plus.circle.fill"
        case .reused: return "arrow.right.circle.fill"
        case .invalidated: return "exclamationmark.circle.fill"
        case .closed: return "xmark.circle.fill"
        case .operator_reset: return "trash.circle.fill"
        case .resume_reused: return "play.circle.fill"
        case .resume_rejected: return "stop.circle.fill"
        case .checkpoint_created: return "arrow.down.doc.fill"
        case .budget_exceeded: return "pedometer.fill"
        case .compacted: return "rectangle.compress.vertical"
        }
    }
    
    private func eventColor(_ type: AgentSessionEventType) -> Color {
        switch type {
        case .created: return .blue
        case .reused: return .green
        case .operator_reset, .budget_exceeded: return .red
        case .invalidated, .compacted: return .orange
        default: return .gray
        }
    }
}
