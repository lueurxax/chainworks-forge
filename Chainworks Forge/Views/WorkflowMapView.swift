import SwiftUI
import SwiftData

struct WorkflowMapView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings
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
            .accessibilityElement(children: .contain)
            .accessibilityLabel(workflowStatusProofLabel(for: projection))
            .accessibilityValue(accessibilitySettingsDescription)
            .overlay(alignment: .topLeading) {
                workflowAccessibilityProofMarkers(for: projection)
            }
        } else {
            ContentUnavailableView(
                "Workflow Map Unavailable",
                systemImage: "chart.xyaxis.line",
                description: Text("This run snapshot could not be rebuilt into a frozen workflow topology.")
            )
            .accessibilityIdentifier("workflow-map-view")
        }
    }

    @ViewBuilder
    private func workflowAccessibilityProofMarkers(for projection: WorkflowMapProjection) -> some View {
        ZStack(alignment: .topLeading) {
            Color.clear
                .frame(width: 1, height: 1)
                .accessibilityElement()
                .accessibilityLabel(workflowStatusProofLabel(for: projection))
                .accessibilityValue(accessibilitySettingsDescription)
                .accessibilityIdentifier("workflow-map-status-proof")

            ForEach(activeAccessibilitySettingIdentifiers, id: \.self) { identifier in
                Color.clear
                    .frame(width: 1, height: 1)
                    .accessibilityElement()
                    .accessibilityLabel(
                        identifier
                            .replacingOccurrences(of: "workflow-map-status-proof-", with: "")
                            .replacingOccurrences(of: "-", with: " ")
                    )
                    .accessibilityIdentifier(identifier)
            }
        }
        .allowsHitTesting(false)
        .opacity(0.001)
    }

    private func workflowStatusProofLabel(for projection: WorkflowMapProjection) -> String {
        let statuses = projection.stages.reduce(into: [String]()) { partialResult, stage in
            let label = stage.status.rawValue
                .replacingOccurrences(of: "_", with: " ")
                .capitalized
            if partialResult.contains(label) == false {
                partialResult.append(label)
            }
        }

        let summary = statuses.isEmpty ? "Unavailable" : statuses.joined(separator: ", ")
        return "Workflow map stage statuses: \(summary)"
    }

    private var accessibilitySettingsDescription: String {
        let activeModes = activeAccessibilitySettingIdentifiers.map {
            $0.replacingOccurrences(of: "workflow-map-status-proof-", with: "")
                .replacingOccurrences(of: "-", with: " ")
        }
        return activeModes.isEmpty ? "standard accessibility display settings" : activeModes.joined(separator: ", ")
    }

    private var activeAccessibilitySettingIdentifiers: [String] {
        var identifiers: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            identifiers.append("workflow-map-status-proof-differentiate-without-color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            identifiers.append("workflow-map-status-proof-increase-contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            identifiers.append("workflow-map-status-proof-reduce-transparency")
        }
        return identifiers
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
                .font(DesignTokens.Typography.supporting)
                .foregroundStyle(DesignTokens.Neutral.textSecondary)
            Text(value)
                .font(DesignTokens.Typography.sectionHeader)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .forgeInsetPanel(tone: .quiet)
    }
}

// MARK: - Topology

private struct WorkflowMapTopologyView: View {
    let projection: WorkflowMapProjection

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForgeSectionHeader(
                title: "Topology",
                subtitle: "Stages remain primary, with transitions and loops grouped under the run rather than competing with it.",
                systemImage: "square.stack.3d.up",
                tint: DesignTokens.Brand.forgeBlueSoft
            )
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
                        .font(DesignTokens.Typography.sectionHeader)
                        .lineLimit(2)
                    Text(stage.ownerAgentTitle)
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
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
            .foregroundStyle(DesignTokens.Neutral.textSecondary)
            .lineLimit(1)

            VStack(alignment: .leading, spacing: 6) {
                Text("\(stage.communicationCount) communications")
                    .font(DesignTokens.Typography.supporting)
                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
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
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    ForEach(stage.transitions) { edge in
                        Text("→ \(edge.toLabel)")
                            .font(.caption2)
                            .foregroundStyle(DesignTokens.Neutral.textSecondary)
                        if let detail = edge.detail, !detail.isEmpty {
                            Text(detail)
                                .font(.caption2)
                                .foregroundStyle(DesignTokens.Neutral.textTertiary)
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
            RoundedRectangle(cornerRadius: DesignTokens.CornerRadius.panel, style: .continuous)
                .fill(stage.isCurrent ? DesignTokens.Neutral.brandWash : DesignTokens.Neutral.surface)
                .shadow(
                    color: DesignTokens.Shadow.cardColor.opacity(isHovered ? 1 : 0.8),
                    radius: DesignTokens.Shadow.cardRadius,
                    y: DesignTokens.Shadow.cardYOffset
                )
        )
        .overlay(
            RoundedRectangle(cornerRadius: DesignTokens.CornerRadius.panel, style: .continuous)
                .stroke(stageCardBorderColor, lineWidth: stage.isCurrent ? 2 : isHovered ? 1.5 : 1)
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

    private var stageCardBorderColor: Color {
        if stage.isCurrent {
            return DesignTokens.Brand.forgeBlueSoft
        }
        if isHovered {
            return DesignTokens.Brand.forgeBlueSoft.opacity(0.45)
        }
        return DesignTokens.Neutral.quietOutline
    }
}

// Proposal 012 (M-01): Migrated to StatusCapsule
private struct WorkflowMapStatusBadge: View {
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings

    let status: WorkflowMapStageState

    private var statusLabel: String {
        status.rawValue.replacingOccurrences(of: "_", with: " ").capitalized
    }

    var body: some View {
        StatusCapsule(
            text: statusLabel,
            color: statusColor,
            accessibilityIdentifier: "workflow-map-status-\(status.rawValue)"
        )
        .overlay(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 1) {
                Color.clear
                    .frame(width: 1, height: 1)
                    .accessibilityElement()
                    .accessibilityLabel(statusLabel)
                    .accessibilityValue(accessibilitySettingsDescription)
                    .accessibilityIdentifier("workflow-map-status-\(status.rawValue)")

                ForEach(activeAccessibilitySettingIdentifiers, id: \.self) { identifier in
                    Color.clear
                        .frame(width: 1, height: 1)
                        .accessibilityIdentifier(identifier)
                }
            }
        }
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

    private var accessibilitySettingsDescription: String {
        var modes: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            modes.append("differentiate without color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            modes.append("increase contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            modes.append("reduce transparency")
        }
        return modes.isEmpty ? "standard accessibility display settings" : modes.joined(separator: ", ")
    }
    private var activeAccessibilitySettingIdentifiers: [String] {
        var identifiers: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            identifiers.append("workflow-map-status-\(status.rawValue)-differentiate-without-color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            identifiers.append("workflow-map-status-\(status.rawValue)-increase-contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            identifiers.append("workflow-map-status-\(status.rawValue)-reduce-transparency")
        }
        return identifiers
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
                .foregroundStyle(DesignTokens.Neutral.textSecondary)
                .lineLimit(2)
            Text("\(occurrence.provider) · \(occurrence.model) · \(occurrence.effort)")
                .font(.caption2)
                .foregroundStyle(DesignTokens.Neutral.textTertiary)
                .lineLimit(1)
        }
        .forgeInsetPanel(tone: .quiet)
    }

    private var panelColor: Color {
        switch occurrence.state {
        case .thinking:
            return DesignTokens.Status.running
        case .completed:
            return DesignTokens.Status.success
        case .notStarted, .ready, .waitingInput:
            return DesignTokens.Status.neutral
        case .failed:
            return DesignTokens.Status.error
        case .skipped:
            return DesignTokens.Status.cancelled
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
                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
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
                                .foregroundStyle(DesignTokens.Neutral.textSecondary)
                            if let detail = edge.detail, !detail.isEmpty {
                                Text(detail)
                                    .font(.caption2)
                                    .foregroundStyle(DesignTokens.Neutral.textTertiary)
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
            return DesignTokens.Status.running
        case .fanout:
            return DesignTokens.Brand.forgeBlueSoft
        case .join:
            return DesignTokens.Status.success
        case .transition:
            return DesignTokens.Status.neutral
        case .loop:
            return DesignTokens.Status.warning
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
            ForgeSectionHeader(
                title: "Agents",
                subtitle: "Active, completed, and pending agents share one vocabulary so runtime state reads consistently across the map.",
                systemImage: "person.3",
                tint: DesignTokens.Brand.forgeBlueSoft
            )
            .accessibilityIdentifier("workflow-map-agents-title")
            GroupBox {
                LazyVGrid(columns: columns, alignment: .leading, spacing: 12) {
                    WorkflowMapAgentPanel(
                        title: "Active",
                        icon: "bolt.fill",
                        tint: DesignTokens.Status.running,
                        tone: .brand,
                        occurrences: activeOccurrences
                    )
                    WorkflowMapAgentPanel(
                        title: "Completed",
                        icon: "checkmark.circle.fill",
                        tint: DesignTokens.Status.success,
                        tone: .success,
                        occurrences: completedOccurrences
                    )
                    WorkflowMapAgentPanel(
                        title: "Pending",
                        icon: "clock",
                        tint: DesignTokens.Status.neutral,
                        tone: .quiet,
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
    let tone: ForgePanelTone
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
                    .font(DesignTokens.Typography.supporting)
                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
            } else {
                ForEach(occurrences) { occurrence in
                    WorkflowMapOccurrenceRow(occurrence: occurrence)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .forgePanel(tone: tone)
    }
}

// MARK: - Loop Telemetry

private struct WorkflowMapLoopTelemetryView: View {
    let projection: WorkflowMapProjection

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForgeSectionHeader(
                title: "Loop Telemetry",
                subtitle: "Loop counters stay secondary to the stage path and surface only the budget and exhaustion state that operators need.",
                systemImage: "repeat",
                tint: DesignTokens.Status.warning
            )
            .accessibilityIdentifier("workflow-map-loops-title")
            GroupBox {
                if projection.loops.isEmpty {
                    Text("No loop counters were declared in the frozen workflow snapshot.")
                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
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
                                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                }
                                ProgressView(value: loop.progress) {
                                    Text(loop.counter)
                                        .font(.caption)
                                } currentValueLabel: {
                                    Text(loop.exhausted ? "Budget reached" : "Budget available")
                                        .font(.caption)
                                }
                                .tint(loop.exhausted ? DesignTokens.Status.warning : DesignTokens.Status.running)
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
        VStack(alignment: .leading, spacing: 8) {
            ForgeSectionHeader(
                title: "Live Timeline",
                subtitle: "Recent workflow events stay concise and subordinate to the current run and stage context.",
                systemImage: "clock.arrow.circlepath",
                tint: DesignTokens.Brand.forgeBlueSoft
            )

            GroupBox {
                if projection.liveTimeline.isEmpty {
                    Text("No live timeline events are available for this run.")
                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
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
                                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                }
                                Text(entry.event.detail)
                                    .font(.caption)
                                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                HStack(spacing: 8) {
                                    Text(entry.stageID)
                                    if let sessionID = entry.event.sessionID {
                                        Text(sessionID)
                                    }
                                    Text(entry.event.timestamp, format: .dateTime.hour().minute().second())
                                }
                                .font(.caption2)
                                .foregroundStyle(DesignTokens.Neutral.textTertiary)
                            }
                            .forgeInsetPanel(tone: .quiet)
                        }
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
