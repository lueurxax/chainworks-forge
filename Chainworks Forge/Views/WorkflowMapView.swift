import SwiftUI
import SwiftData

struct WorkflowMapView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(ExecutionService.self) private var executionService

    let run: Run

    private var projection: WorkflowMapProjection? {
        let service = WorkflowMapProjectionService(
            modelContext: modelContext,
            executionService: executionService
        )
        return service.projection(for: run)
    }

    var body: some View {
        if let projection {
            VStack(alignment: .leading, spacing: 14) {
                WorkflowMapSummaryStrip(projection: projection)
                    .accessibilityIdentifier("workflow-map-summary")
                WorkflowMapTopologyView(projection: projection)
                    .accessibilityIdentifier("workflow-map-topology")
                WorkflowMapHandoffLedger(projection: projection)
                    .accessibilityIdentifier("workflow-map-handoffs")
                WorkflowMapAgentPanels(projection: projection)
                    .accessibilityIdentifier("workflow-map-agents")
                WorkflowMapLoopTelemetryView(projection: projection)
                    .accessibilityIdentifier("workflow-map-loops")
                WorkflowMapTimelineView(projection: projection)
                    .accessibilityIdentifier("workflow-map-timeline")
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier("workflow-map-view")
        } else {
            ContentUnavailableView(
                "Workflow Map Unavailable",
                systemImage: "chart.xyaxis.line",
                description: Text("This run snapshot could not be rebuilt into a frozen workflow topology.")
            )
            .accessibilityIdentifier("workflow-map-view")
        }
    }
}

// MARK: - Summary Strip

private struct WorkflowMapSummaryStrip: View {
    let projection: WorkflowMapProjection

    var body: some View {
        Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 8) {
            GridRow {
                WorkflowMapStatChip(label: "Run", value: projection.runStatus.rawValue.capitalized, systemImage: "chart.bar")
                WorkflowMapStatChip(label: "Stage", value: projection.currentStageLabel ?? "Not started", systemImage: "square.stack.3d.up")
                WorkflowMapStatChip(label: "Communications", value: "\(projection.communicationCount)", systemImage: "arrow.left.arrow.right")
                WorkflowMapStatChip(label: "Live events", value: "\(projection.liveEventCount)", systemImage: "dot.radiowaves.left.and.right")
            }
            GridRow {
                WorkflowMapStatChip(label: "Active agents", value: "\(projection.activeOccurrenceCount)", systemImage: "bolt.fill")
                WorkflowMapStatChip(label: "Completed", value: "\(projection.completedOccurrenceCount)", systemImage: "checkmark.circle.fill")
                WorkflowMapStatChip(label: "Pending", value: "\(projection.pendingOccurrenceCount)", systemImage: "clock")
                WorkflowMapStatChip(label: "Failed", value: "\(projection.failedOccurrenceCount)", systemImage: "xmark.circle.fill")
            }
        }
    }
}

private struct WorkflowMapStatChip: View {
    let label: String
    let value: String
    let systemImage: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(label, systemImage: systemImage)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.headline)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

// MARK: - Topology

private struct WorkflowMapTopologyView: View {
    let projection: WorkflowMapProjection

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Topology")
                .font(.headline)
                .accessibilityIdentifier("workflow-map-topology-title")
            GroupBox {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(alignment: .top, spacing: 12) {
                        ForEach(projection.stages.indices, id: \.self) { index in
                            let stage = projection.stages[index]
                            if index > 0 {
                                Image(systemName: "chevron.right")
                                    .foregroundStyle(.secondary)
                                    .padding(.top, 34)
                            }
                            WorkflowMapStageCard(stage: stage)
                        }
                    }
                    .padding(.vertical, 4)
                }
            }
        }
    }
}

// Proposal 012 (L-05): Interactive stage cards with hover + popover
private struct WorkflowMapStageCard: View {
    let stage: WorkflowMapStageProjection
    @State private var isHovered = false
    @State private var showPopover = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(stage.label)
                        .font(.headline)
                        .lineLimit(2)
                    Text(stage.ownerAgentTitle)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                WorkflowMapStatusBadge(status: stage.status)
            }

            HStack(spacing: 8) {
                Label("Iter \(stage.iteration)", systemImage: "arrow.triangle.2.circlepath")
                if stage.approvalRequired {
                    Label("Approval", systemImage: "checkmark.seal")
                }
                if let loopTelemetry = stage.loopTelemetry {
                    Label("\(loopTelemetry.counter): \(loopTelemetry.current)/\(loopTelemetry.max)", systemImage: "repeat")
                }
            }
            .font(.caption2)
            .foregroundStyle(.secondary)
            .lineLimit(1)

            VStack(alignment: .leading, spacing: 6) {
                Text("\(stage.communicationCount) communications")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                ForEach(Array(stage.occurrences.prefix(3))) { occurrence in
                    WorkflowMapOccurrenceRow(occurrence: occurrence)
                }
                if stage.occurrences.count > 3 {
                    Text("+ \(stage.occurrences.count - 3) more")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }

            if !stage.transitions.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Transitions")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(stage.transitions) { edge in
                        Text("→ \(edge.toLabel)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        if let detail = edge.detail, !detail.isEmpty {
                            Text(detail)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                    }
                }
            }

            if let loopTelemetry = stage.loopTelemetry {
                ProgressView(value: loopTelemetry.progress) {
                    Text("Loop progress")
                        .font(.caption2)
                } currentValueLabel: {
                    Text("\(loopTelemetry.current)/\(loopTelemetry.max)")
                        .font(.caption2)
                }
                .tint(loopTelemetry.exhausted ? .orange : .blue)
            }
        }
        .padding(12)
        .frame(width: 250, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(.background)
                .shadow(color: .black.opacity(0.04), radius: 4, y: 2)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(stage.isCurrent ? Color.accentColor : isHovered ? Color.accentColor.opacity(0.5) : Color.secondary.opacity(0.18), lineWidth: stage.isCurrent ? 2 : isHovered ? 1.5 : 1)
        )
        // Proposal 012 (L-05): Hover effect and tap popover
        .onHover { hovering in
            withAnimation(.easeInOut(duration: 0.15)) { isHovered = hovering }
        }
        .scaleEffect(isHovered ? 1.02 : 1.0)
        .animation(.easeInOut(duration: 0.15), value: isHovered)
        .onTapGesture { showPopover = true }
        .popover(isPresented: $showPopover, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: DesignTokens.Spacing.small) {
                Text(stage.label)
                    .font(DesignTokens.Typography.sectionHeader)
                Divider()
                LabeledContent("Owner", value: stage.ownerAgentTitle)
                LabeledContent("Status", value: stage.status.rawValue.replacingOccurrences(of: "_", with: " ").capitalized)
                LabeledContent("Iteration", value: "\(stage.iteration)")
                if stage.approvalRequired {
                    Label("Approval required", systemImage: "checkmark.seal")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(DesignTokens.Status.warning)
                }
                LabeledContent("Communications", value: "\(stage.communicationCount)")
                LabeledContent("Agent Occurrences", value: "\(stage.occurrences.count)")
                if !stage.transitions.isEmpty {
                    Text("Transitions: \(stage.transitions.map(\.toLabel).joined(separator: ", "))")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(.secondary)
                }
            }
            .padding()
            .frame(minWidth: 280)
        }
    }
}

// Proposal 012 (M-01): Migrated to StatusCapsule
private struct WorkflowMapStatusBadge: View {
    let status: WorkflowMapStageState

    var body: some View {
        StatusCapsule(
            text: status.rawValue.replacingOccurrences(of: "_", with: " ").capitalized,
            color: statusColor,
            accessibilityIdentifier: "workflow-map-status-\(status.rawValue)"
        )
    }

    private var statusColor: Color {
        switch status {
        case .notStarted, .pending, .ready:
            return DesignTokens.Status.neutral
        case .running:
            return DesignTokens.Status.running
        case .waitingApproval:
            return DesignTokens.Status.warning
        case .blocked, .failed:
            return DesignTokens.Status.error
        case .completed:
            return DesignTokens.Status.success
        case .skipped:
            return DesignTokens.Status.cancelled
        }
    }
}

private struct WorkflowMapOccurrenceRow: View {
    let occurrence: WorkflowMapOccurrenceProjection

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(occurrence.agentTitle)
                    .font(.caption.weight(.semibold))
                Spacer()
                Text(occurrence.state.rawValue.capitalized)
                    .font(.caption2)
                    .foregroundStyle(panelColor)
            }
            Text(occurrence.taskName)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
            Text("\(occurrence.provider) · \(occurrence.model) · \(occurrence.effort)")
                .font(.caption2)
                .foregroundStyle(.tertiary)
                .lineLimit(1)
        }
        .padding(8)
        .background(Color.secondary.opacity(0.07), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private var panelColor: Color {
        switch occurrence.state {
        case .thinking:
            return .blue
        case .completed:
            return .green
        case .notStarted, .ready, .waitingInput:
            return .secondary
        case .failed:
            return .red
        case .skipped:
            return .gray
        }
    }
}

// MARK: - Handoffs

private struct WorkflowMapHandoffLedger: View {
    let projection: WorkflowMapProjection

    private var edges: [WorkflowMapEdge] {
        projection.edges.sorted { lhs, rhs in
            if lhs.kind.rawValue == rhs.kind.rawValue {
                return lhs.fromLabel < rhs.fromLabel
            }
            return lhs.kind.rawValue < rhs.kind.rawValue
        }
    }

    var body: some View {
        GroupBox("Handoffs") {
            if edges.isEmpty {
                Text("No handoff edges were derived from the frozen workflow snapshot.")
                    .foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(edges) { edge in
                        HStack(alignment: .firstTextBaseline, spacing: 10) {
                            Text(edge.fromLabel)
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(1)
                            Image(systemName: handoffIcon(for: edge.kind))
                                .foregroundStyle(color(for: edge.kind))
                            Text(edge.toLabel)
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(1)
                            Spacer()
                            Text(edge.kind.rawValue.replacingOccurrences(of: "_", with: " "))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            if let detail = edge.detail, !detail.isEmpty {
                                Text(detail)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                            }
                        }
                        .padding(.vertical, 2)
                    }
                }
            }
        }
    }

    private func handoffIcon(for kind: WorkflowMapEdgeKind) -> String {
        switch kind {
        case .sequence:
            return "arrow.right"
        case .fanout:
            return "arrow.up.right"
        case .join:
            return "arrow.down.right"
        case .transition:
            return "arrow.turn.up.right"
        case .loop:
            return "repeat"
        }
    }

    private func color(for kind: WorkflowMapEdgeKind) -> Color {
        switch kind {
        case .sequence:
            return .blue
        case .fanout:
            return .indigo
        case .join:
            return .green
        case .transition:
            return .secondary
        case .loop:
            return .orange
        }
    }
}

// MARK: - Agent Panels

private struct WorkflowMapAgentPanels: View {
    let projection: WorkflowMapProjection

    private var activeOccurrences: [WorkflowMapOccurrenceProjection] {
        projection.activeOccurrences
    }

    private var completedOccurrences: [WorkflowMapOccurrenceProjection] {
        projection.completedOccurrences
    }

    private var pendingOccurrences: [WorkflowMapOccurrenceProjection] {
        projection.pendingOccurrences
    }

    var body: some View {
        let columns = [
            GridItem(.flexible(), spacing: 12),
            GridItem(.flexible(), spacing: 12),
            GridItem(.flexible(), spacing: 12)
        ]

        VStack(alignment: .leading, spacing: 8) {
            Text("Agents")
                .font(.headline)
                .accessibilityIdentifier("workflow-map-agents-title")
            GroupBox {
                LazyVGrid(columns: columns, alignment: .leading, spacing: 12) {
                    WorkflowMapAgentPanel(
                        title: "Active",
                        icon: "bolt.fill",
                        tint: .blue,
                        occurrences: activeOccurrences
                    )
                    WorkflowMapAgentPanel(
                        title: "Completed",
                        icon: "checkmark.circle.fill",
                        tint: .green,
                        occurrences: completedOccurrences
                    )
                    WorkflowMapAgentPanel(
                        title: "Pending",
                        icon: "clock",
                        tint: .secondary,
                        occurrences: pendingOccurrences
                    )
                }
            }
        }
    }
}

private struct WorkflowMapAgentPanel: View {
    let title: String
    let icon: String
    let tint: Color
    let occurrences: [WorkflowMapOccurrenceProjection]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label(title, systemImage: icon)
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Text("\(occurrences.count)")
                    .font(.caption.bold())
                    .foregroundStyle(tint)
            }

            if occurrences.isEmpty {
                Text("No \(title.lowercased()) agents.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(occurrences) { occurrence in
                    WorkflowMapOccurrenceRow(occurrence: occurrence)
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.secondary.opacity(0.05), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

// MARK: - Loop Telemetry

private struct WorkflowMapLoopTelemetryView: View {
    let projection: WorkflowMapProjection

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Loop Telemetry")
                .font(.headline)
                .accessibilityIdentifier("workflow-map-loops-title")
            GroupBox {
                if projection.loops.isEmpty {
                    Text("No loop counters were declared in the frozen workflow snapshot.")
                        .foregroundStyle(.secondary)
                } else {
                    VStack(alignment: .leading, spacing: 10) {
                        ForEach(projection.loops) { loop in
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Text(loop.stageLabel)
                                        .font(.subheadline.weight(.semibold))
                                    Spacer()
                                    Text("\(loop.current)/\(loop.max)")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                ProgressView(value: loop.progress) {
                                    Text(loop.counter)
                                        .font(.caption)
                                } currentValueLabel: {
                                    Text(loop.exhausted ? "Budget reached" : "Budget available")
                                        .font(.caption)
                                }
                                .tint(loop.exhausted ? .orange : .blue)
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: - Timeline

private struct WorkflowMapTimelineView: View {
    let projection: WorkflowMapProjection

    var body: some View {
        GroupBox("Live Timeline") {
            if projection.liveTimeline.isEmpty {
                Text("No live timeline events are available for this run.")
                    .foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(projection.liveTimeline.prefix(8))) { entry in
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
                        .padding(.vertical, 2)
                    }
                }
            }
        }
    }
}

#Preview("Workflow Map — Proposal Loop") {
    let container = PreviewSupport.makeModelContainer(seed: { context in
        PreviewSupport.seedWorkflowMapPreviewData(context: context)
    })
    let executionService = PreviewSupport.makeExecutionService(modelContext: container.mainContext)
    let run: Run = {
        let descriptor = FetchDescriptor<Run>()
        do {
            let runs = try container.mainContext.fetch(descriptor)
            guard let run = runs.first else {
                fatalError("Seeded run not available")
            }
            return run
        } catch {
            fatalError("Seeded run not available: \(error.localizedDescription)")
        }
    }()

    WorkflowMapView(run: run)
        .modelContainer(container)
        .environment(executionService)
        .padding()
        .frame(width: 1280, height: 900)
}
