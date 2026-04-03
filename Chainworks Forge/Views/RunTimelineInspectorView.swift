import SwiftUI

struct RunTimelineInspectorView: View {
    let projection: WorkflowMapProjection
    var showsTitle: Bool = true

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if showsTitle {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Live Timeline")
                            .font(.title3.weight(.semibold))
                        Text("Focused run timeline inspection for live events and persisted checkpoints.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                if projection.liveTimeline.isEmpty, projection.persistedTimeline.isEmpty {
                    ContentUnavailableView(
                        "No Timeline Data",
                        systemImage: "waveform.path.ecg",
                        description: Text("This run has no in-memory or persisted timeline entries yet.")
                    )
                } else {
                    if projection.liveTimeline.isEmpty == false {
                        GroupBox("Live Stream") {
                            VStack(alignment: .leading, spacing: 10) {
                                ForEach(projection.liveTimeline) { entry in
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(entry.agentTitle)
                                                .font(.subheadline.weight(.semibold))
                                            Spacer()
                                            Text(entry.event.type.rawValue)
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }

                                        Text(entry.event.detail)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)

                                        HStack(spacing: 8) {
                                            Text(entry.stageID)
                                            if let sessionID = entry.event.sessionID {
                                                Text(sessionID)
                                            }
                                            Text(entry.event.timestamp, format: .dateTime.hour().minute().second())
                                        }
                                        .font(.caption2)
                                        .foregroundStyle(.tertiary)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(10)
                                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                                }
                            }
                        }
                    }

                    if projection.persistedTimeline.isEmpty == false {
                        GroupBox("Persisted Checkpoints") {
                            VStack(alignment: .leading, spacing: 10) {
                                ForEach(projection.persistedTimeline) { entry in
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(entry.title)
                                                .font(.subheadline.weight(.semibold))
                                            Spacer()
                                            Text(entry.timestamp, format: .dateTime.hour().minute().second())
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }
                                        Text(entry.detail)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .padding(10)
                                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                                }
                            }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding()
        }
        .frame(minWidth: 480, minHeight: 420)
        .accessibilityIdentifier("run-timeline-inspector-view")
    }
}
