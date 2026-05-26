import Foundation
import Combine

/// P036 RunsWorkbenchPresentationModel
/// A typed presenter/view-model layer normalizes GraphQL and P085 inputs into SwiftUI presentation rows before views render them.
@MainActor
final class RunsWorkbenchPresentationModel: ObservableObject {
    // MARK: - Outputs
    @Published private(set) var sidebarLanes: [SidebarLane] = []
    @Published private(set) var summaryHeader: SummaryHeader?
    @Published private(set) var stageMap: StageMap?
    @Published private(set) var inlineApprovals: [ApprovalRow] = []
    @Published private(set) var artifactsAndReports: [ArtifactReportRow] = []
    @Published private(set) var reportRows: [P031ReportMetadataRowPresentation] = []
    @Published private(set) var recoveryEvidence: [RecoveryEvidenceRow] = []
    @Published private(set) var freshnessAndHealth: FreshnessHealth?
    @Published private(set) var timelineEntries: [TimelineEntry] = []
    @Published private(set) var activeTimelineAgents: [ActiveTimelineAgent] = []
    @Published private(set) var selectedActiveTimelineAgentID: String?
    @Published private(set) var approvalInbox: P031ApprovalInboxPresentation?
    @Published private(set) var deferredStates: [DeferredStateRow] = []

    @Published private(set) var ideaContext: P031IdeaContextPresentation?
    @Published private(set) var catalogContext: P031CatalogContextPresentation?
    @Published private(set) var closeoutReadiness: P077CloseoutReadinessPresentation?
    @Published private(set) var implementationCompletion: P088ImplementationCompletionPresentation?
    @Published private(set) var sideEffectReadback: P078SideEffectReadbackPresentation?

    // PC-003 routing: set true when ContentView routes approvals → Runs. RunsHomeView
    // reads this on mount (initial: true) so the flag survives the tab-switch render cycle
    // where a synchronous NotificationCenter post would be lost before the view is mounted.
    @Published private(set) var pendingFocusWaitingApprovalLane: Bool = false

    init() {}

    func requestFocusWaitingApprovalLane() {
        pendingFocusWaitingApprovalLane = true
    }

    func clearFocusWaitingApprovalLane() {
        pendingFocusWaitingApprovalLane = false
    }

    func selectActiveTimelineAgent(_ agentID: String?) {
        selectedActiveTimelineAgentID = agentID
    }

    // MARK: - Integration
    func populate(from runsHome: P031RunsHomePresentation) {
        var waiting: [P031RunsHomeRowPresentation] = []
        var blocked: [P031RunsHomeRowPresentation] = []
        var running: [P031RunsHomeRowPresentation] = []
        var completed: [P031RunsHomeRowPresentation] = []

        var deferred: [P031RunsHomeRowPresentation] = []
        for row in runsHome.rows {
            switch row.lane {
            case .waiting:
                waiting.append(row)
            case .blocked:
                blocked.append(row)
            case .running:
                running.append(row)
            case .completed:
                completed.append(row)
            case .deferred:
                deferred.append(row)
            }
        }

        sidebarLanes = [
            SidebarLane(id: "waiting", title: "Waiting approval", runs: waiting),
            SidebarLane(id: "blocked", title: "Blocked or failed", runs: blocked),
            SidebarLane(id: "running", title: "Running", runs: running),
            SidebarLane(id: "completed", title: "Recently completed", runs: completed),
            SidebarLane(id: "deferred", title: "Status Unknown", runs: deferred)
        ].filter { !$0.runs.isEmpty }
    }

    func populate(from detail: P031RunDetailPresentation) {
        summaryHeader = SummaryHeader(
            title: detail.title,
            runID: detail.runID,
            status: detail.statusLabel,
            workflowLabel: detail.workflowLabel,
            progressLabel: detail.progressLabel,
            pendingApprovalsLabel: detail.pendingApprovalsLabel,
            rolloutDecisionSummary: detail.rolloutDecisionSummary.map { "Rollout \($0.backendDecision)" },
            refreshFeedbackText: detail.refreshFeedbackText,
            errorDescription: detail.errorDescription,
            freshness: detail.freshness.state.rawValue
        )

        stageMap = StageMap(
            stages: detail.stageTopology.map { stage in
                return StageCard(
                    id: stage.stageID,
                    ordinal: stage.ordinal,
                    title: stage.title,
                    ownerAgentTitle: stage.ownerAgentTitle,
                    status: Self.stageTopologyStatus(for: stage.status),
                    statusText: stage.statusText,
                    isCurrent: stage.isCurrent,
                    iterationText: stage.iterationText,
                    attemptText: stage.attemptText,
                    startedLabel: nil,
                    completedLabel: nil,
                    durationLabel: nil,
                    evidenceLabels: Self.stageTopologyEvidenceLabels(for: stage),
                    artifactCount: stage.artifactCount,
                    communicationCount: stage.communicationCount,
                    approvalRequired: stage.approvalRequired,
                    occurrences: stage.occurrences.prefix(3).map { occurrence in
                        StageOccurrence(
                            id: "\(stage.stageID)-\(occurrence.agentID)-\(occurrence.taskName)",
                            agentTitle: occurrence.agentTitle,
                            taskName: occurrence.taskName,
                            statusText: occurrence.statusText,
                            providerLabel: occurrence.providerLabel,
                            executionCountLabel: occurrence.executionCountLabel
                        )
                    },
                    hiddenOccurrenceCount: max(0, stage.occurrences.count - 3),
                    transitions: stage.transitions.map { transition in
                        StageTransition(
                            id: "\(stage.stageID)-\(transition.toStageID)",
                            toLabel: transition.toLabel,
                            detail: transition.detail
                        )
                    }
                )
            }
        )

        let mappedApprovals = detail.approvalRows.map { row in
            let affordance = row.affordance

            // P036-SEC-005: compute deferred state FIRST so it gates canApprove/canReject.
            // Freshness/authz/deferred rows must never render enabled action buttons regardless
            // of what P085 affordance availability reports.
            let deferred: P036DeferredState? = {
                // P085 state matrix mapping: FRESHNESS takes precedence for deferred state
                // P036-SEC-004: .unknown and .refreshing map to .unsupported so the row always
                // shows a banner instead of silently rendering with no buttons and no explanation.
                switch affordance.freshnessState {
                case .projectionLag: return .projectionLag
                case .stale: return .stale
                case .unauthorized: return .unauthorized
                case .unavailable: return .unavailable
                case .refreshing: return .unavailable
                case .unknown(rawValue: _): return .unsupported
                default: break
                }

                // Fallback to disabled reason codes for contract-driven deferred states.
                // Check both approve and reject so reject-only disabled states (e.g. redacted,
                // conflict) are also surfaced as explicit deferred rows.
                for availability in [affordance.approveAvailability, affordance.rejectAvailability] {
                    if case .disabled(let code, _) = availability, let code {
                        switch code {
                        case .unauthorized: return .unauthorized
                        case .staleRead: return .stale
                        case .projectionLag: return .projectionLag
                        case .managedOutsideUI, .unsupportedAction, .ambiguousApprovalIdentity: return .unsupported
                        case .redacted: return .redacted
                        case .conflict: return .conflict
                        case .duplicate: return .duplicate
                        case .alreadyResolved: return .alreadyResolved
                        case .approvalNotActionable: return .alreadyResolved
                        case .observerScope: return .unauthorized
                        case .nonApprovalMutation, .capabilityOutOfScope: return .unsupported
                        case .writePathNotAvailable: return .unavailable
                        }
                    }
                }

                return nil
            }()

            // Fail closed: any non-nil deferred state disables both action buttons.
            let canApprove: Bool = {
                guard deferred == nil else { return false }
                if case .actionable = affordance.approveAvailability { return true }
                return false
            }()

            let canReject: Bool = {
                guard deferred == nil else { return false }
                if case .actionable = affordance.rejectAvailability { return true }
                return false
            }()

            let approveDisabledReason: String? = {
                if case .disabled(_, let helpText) = affordance.approveAvailability { return helpText }
                return nil
            }()

            let rejectDisabledReason: String? = {
                if case .disabled(_, let helpText) = affordance.rejectAvailability { return helpText }
                return nil
            }()

            // M2: When deferredState == .redacted, substitute a generic message rather than
            // leaking the raw helpText from the server (which may contain detail about what
            // was redacted). The generic message still communicates why buttons are disabled.
            let redactedMessage = "Redacted — details unavailable"
            // M3: When state is .redacted, body text may also contain sensitive detail; suppress it.
            let bodyText: String? = {
                if deferred == .redacted { return nil }
                let trimmed = row.body.trimmingCharacters(in: .whitespacesAndNewlines)
                return trimmed.isEmpty ? nil : trimmed
            }()
            return ApprovalRow(
                id: row.approvalID,
                title: row.title,
                body: bodyText,
                canApprove: canApprove,
                canReject: canReject,
                approveDisabledReason: deferred == .redacted ? redactedMessage : approveDisabledReason,
                rejectDisabledReason: deferred == .redacted ? redactedMessage : rejectDisabledReason,
                deferredState: deferred,
                // M4 (P036-SEC-001): upstream accessibilityLabel may encode sensitive approval
                // body text before the projection layer applies redaction. Substitute a generic
                // label so VoiceOver / assistive tech cannot read redacted content.
                accessibilityLabel: deferred == .redacted
                    ? "Approval pending review — details restricted"
                    : row.accessibilityLabel,
                // PC-001: suppress follow-up ID and copy items when redacted so sensitive
                // diagnostic context is not surfaced via these alternate channels.
                followUpID: deferred == .redacted ? nil : row.followUpID,
                copyItems: deferred == .redacted ? [] : row.copyItems
            )
        }
        inlineApprovals = mappedApprovals
        if !mappedApprovals.isEmpty {
            let actionabilityState = mappedApprovals.first.map { $0.canApprove ? "actionable" : "disabled" } ?? "disabled"
            let freshnessStr = detail.freshness.state.rawValue
            P036UICounters.shared.recordInlineApprovalRender(
                count: mappedApprovals.count,
                freshnessState: freshnessStr,
                actionabilityState: actionabilityState
            )
        }

        artifactsAndReports = detail.artifactViewerRows.map { row in
            let availability: ArtifactPayloadAvailability = {
                switch row.payloadState {
                case .available: return .available
                case .metadataOnly: return .metadataOnly
                case .payloadDeferred: return .deferred
                case .generating: return .generating
                case .unavailable: return .unavailable
                }
            }()
            return ArtifactReportRow(
                id: row.artifactID,
                title: row.title,
                payloadAvailability: availability,
                presentation: row
            )
        }
        if !artifactsAndReports.isEmpty {
            P036UICounters.shared.recordArtifactPayloadState(
                count: artifactsAndReports.count,
                payloadAvailabilityState: "mixed",
                renderKind: "workbench_row_batch"
            )
        }

        reportRows = detail.reportRows

        recoveryEvidence = (detail.closeoutReadiness?.diagnosticRows ?? []).enumerated().map {
            RecoveryEvidenceRow(id: "recovery-\($0.offset)", title: $0.element)
        }

        freshnessAndHealth = FreshnessHealth(
            freshness: detail.freshness.state.rawValue,
            daemonHealth: freshnessAndHealth?.daemonHealth ?? "Unknown",
            schedulerHealth: freshnessAndHealth?.schedulerHealth,
            mcpHubStatus: freshnessAndHealth?.mcpHubStatus ?? "Unknown",
            capabilitiesStatus: freshnessAndHealth?.capabilitiesStatus ?? "Pending",
            isSystemReady: freshnessAndHealth?.isSystemReady ?? false,
            isReadinessDeferred: freshnessAndHealth?.isReadinessDeferred ?? true
        )

        ideaContext = detail.ideaContext
        catalogContext = detail.catalogContext
        closeoutReadiness = detail.closeoutReadiness
        implementationCompletion = detail.implementationCompletion
        sideEffectReadback = detail.sideEffectReadback
        activeTimelineAgents = detail.activeAgentTimelineEntries.map { entry in
            ActiveTimelineAgent(
                id: entry.agentID,
                title: entry.title,
                providerID: entry.providerID,
                stageID: entry.stageID,
                stageLabel: entry.stageLabel,
                taskLabel: entry.taskLabel,
                status: entry.status,
                sessionID: entry.sessionID,
                latestAt: entry.timestamp,
                eventCount: entry.eventCount ?? 0,
                selectionOrder: entry.selectionOrder,
                selectionUnavailableReason: entry.selectionUnavailableReason
            )
        }
        timelineEntries = []

        if let error = detail.errorDescription {
            deferredStates = [
                DeferredStateRow(id: "error", summary: error, state: .unavailable)
            ]
        } else {
            deferredStates = []
        }
    }

    private static func entriesForFocusedActiveAgent(
        _ entries: [FocusedTimelineSpineEntry],
        selectedAgentID: String?
    ) -> [FocusedTimelineSpineEntry] {
        let completedAgentIDs = Set(
            entries.compactMap { entry -> String? in
                guard entry.kind == .agentSummary else { return nil }
                return entry.agentID
            }
        )
        let activeEntries = entries.filter { entry in
            guard let agentID = entry.agentID else { return false }
            return !completedAgentIDs.contains(agentID)
        }
        let resolvedAgentID: String? = {
            if let selectedAgentID,
               activeEntries.contains(where: { $0.agentID == selectedAgentID }) {
                return selectedAgentID
            }
            return activeEntries
                .max(by: { $0.timestamp < $1.timestamp })?
                .agentID
        }()
        guard let resolvedAgentID else { return [] }
        return activeEntries.filter { $0.agentID == resolvedAgentID }
    }

    func populate(from inbox: P031ApprovalInboxPresentation) {
        self.approvalInbox = inbox
    }

    func populate(daemon: P031DaemonLifecyclePresentation?, scheduler: SchedulerHealthReadback?) {
        let daemonLabel = daemon?.state?.rawValue.capitalized ?? "Unavailable"
        let schedulerLabel: String? = {
            guard let scheduler = scheduler?.health else { return nil }
            return "Queued: \(scheduler.queuedCount) / Active: \(scheduler.activeAgentExecutions)"
        }()

        let mcpHubLabel: String = {
            guard let scheduler = scheduler?.health else { return "Unknown" }
            if scheduler.isStale { return "Disconnected" }
            return "Connected"
        }()

        let capabilitiesLabel: String = {
            guard let scheduler = scheduler else { return "Pending" }
            return scheduler.activeProviders.isEmpty ? "Unavailable" : "Validated"
        }()

        // Readiness is deferred when daemon projection hasn't arrived; do not infer false.
        let isReadinessDeferred = daemon == nil

        let isReady: Bool = {
            guard let daemonState = daemon?.state else { return false }
            guard daemonState == .ready else { return false }
            guard let scheduler = scheduler?.health else { return true }
            let state = scheduler.sustainedBackpressureState.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            return ["", "none", "clear", "healthy", "idle"].contains(state) && scheduler.oldestQueuedAgeMs < 5 * 60 * 1000
        }()

        freshnessAndHealth = FreshnessHealth(
            freshness: freshnessAndHealth?.freshness ?? "unknown",
            daemonHealth: daemonLabel,
            schedulerHealth: schedulerLabel,
            mcpHubStatus: mcpHubLabel,
            capabilitiesStatus: capabilitiesLabel,
            isSystemReady: isReady,
            isReadinessDeferred: isReadinessDeferred
        )
    }

    private nonisolated static func stageTopologyStatus(for rawStatus: String) -> String {
        let status = rawStatus.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if status.contains("complete") || status == "succeeded" || status == "approved" {
            return "terminal"
        }
        if status.contains("run") || status.contains("active") || status.contains("progress") {
            return "active"
        }
        if status.contains("block") || status.contains("fail") {
            return "blocked"
        }
        if status.contains("pending") || status.contains("waiting") {
            return "pending"
        }
        return "unavailable"
    }

    private nonisolated static func stageTopologyEvidenceLabels(
        for stage: P031StageTopologyPresentation
    ) -> [String] {
        [
            stage.approvalRequired ? "Approval" : nil,
            stage.artifactCount > 0 ? "\(stage.artifactCount) artifact\(stage.artifactCount == 1 ? "" : "s")" : nil,
            stage.communicationCount > 0 ? "\(stage.communicationCount) signals" : nil
        ].compactMap { $0 }
    }

    // MARK: - Supporting Types
    struct SidebarLane: Identifiable, Equatable {
        let id: String
        let title: String
        let runs: [P031RunsHomeRowPresentation]
    }

    struct SummaryHeader: Equatable {
        let title: String
        let runID: String?
        let status: String
        let workflowLabel: String?
        let progressLabel: String?
        let pendingApprovalsLabel: String?
        let rolloutDecisionSummary: String?
        let refreshFeedbackText: String?
        let errorDescription: String?
        let freshness: String
    }

    struct StageMap: Equatable {
        let stages: [StageCard]
        let layoutColumns: [StageTopologyColumn]
        let connectorColumns: [StageTopologyConnectorColumn]

        init(stages: [StageCard]) {
            self.stages = stages
            let layout = StageTopologyLayoutBuilder.layout(for: stages)
            self.layoutColumns = layout.columns
            self.connectorColumns = layout.connectors
        }
    }

    struct StageCard: Identifiable, Equatable {
        let id: String
        let ordinal: Int
        let title: String
        let ownerAgentTitle: String
        let status: String
        let statusText: String
        let isCurrent: Bool
        let iterationText: String?
        let attemptText: String?
        let startedLabel: String?
        let completedLabel: String?
        let durationLabel: String?
        let evidenceLabels: [String]
        let artifactCount: Int
        let communicationCount: Int
        let approvalRequired: Bool
        let occurrences: [StageOccurrence]
        let hiddenOccurrenceCount: Int
        let transitions: [StageTransition]
    }

    struct StageTopologyColumn: Identifiable, Equatable {
        let id: String
        let title: String
        let slots: [StageTopologySlot]
    }

    struct StageTopologySlot: Identifiable, Equatable {
        enum Kind: Equatable {
            case stage
            case bridge
        }

        let id: String
        let kind: Kind
        let stage: StageCard?
        let bridgeLabel: String?
        let heightUnits: Int

        static func stage(_ stage: StageCard, heightUnits: Int = 1) -> StageTopologySlot {
            StageTopologySlot(
                id: "slot-\(stage.id)",
                kind: .stage,
                stage: stage,
                bridgeLabel: nil,
                heightUnits: max(1, heightUnits)
            )
        }

        static func bridge(id: String, label: String, heightUnits: Int = 1) -> StageTopologySlot {
            StageTopologySlot(
                id: id,
                kind: .bridge,
                stage: nil,
                bridgeLabel: label,
                heightUnits: max(1, heightUnits)
            )
        }
    }

    struct StageTopologyConnectorColumn: Identifiable, Equatable {
        let id: String
        let connectors: [StageTopologyConnector]
    }

    struct StageTopologyConnector: Identifiable, Equatable {
        enum Style: Equatable {
            case primary
            case retry
            case manual
            case hidden
        }

        let id: String
        let style: Style
    }

    struct StageOccurrence: Identifiable, Equatable {
        let id: String
        let agentTitle: String
        let taskName: String
        let statusText: String
        let providerLabel: String
        let executionCountLabel: String?
    }

    struct StageTransition: Identifiable, Equatable {
        let id: String
        let toLabel: String
        let detail: String?
    }

    private struct StageTopologyLayoutBuilder {
        struct Layout {
            let columns: [StageTopologyColumn]
            let connectors: [StageTopologyConnectorColumn]
        }

        private static let fullMVPOrder = [
            "state_1_idea_received",
            "state_2_proposal_drafted",
            "state_3_initial_proposal_approval",
            "state_4_proposal_reviewed",
            "state_5_proposal_refined",
            "state_6_implementation_approval",
            "state_7_implementation_started",
            "state_8_implementation_continued",
            "state_9_implementation_reviewed",
            "state_10_implementation_refined",
            "state_11_manual_release",
            "state_12_workflow_complete"
        ]

        static func layout(for stages: [StageCard]) -> Layout {
            let byID = Dictionary(uniqueKeysWithValues: stages.map { ($0.id, $0) })
            if fullMVPOrder.allSatisfy({ byID[$0] != nil }) {
                return fullMVPLayout(byID: byID)
            }
            return sequentialLayout(for: stages)
        }

        private static func fullMVPLayout(byID: [String: StageCard]) -> Layout {
            func stage(_ id: String, heightUnits: Int = 1) -> StageTopologySlot {
                StageTopologySlot.stage(byID[id]!, heightUnits: heightUnits)
            }
            func column(_ id: String, _ title: String, _ slots: [StageTopologySlot]) -> StageTopologyColumn {
                StageTopologyColumn(id: "column-\(id)", title: title, slots: slots)
            }
            func connector(
                _ id: String,
                _ styles: [StageTopologyConnector.Style]
            ) -> StageTopologyConnectorColumn {
                StageTopologyConnectorColumn(
                    id: "connector-\(id)",
                    connectors: styles.enumerated().map { index, style in
                        StageTopologyConnector(id: "connector-\(id)-\(index)", style: style)
                    }
                )
            }

            let columns = [
                column("intake", "Intake", [
                    stage("state_1_idea_received")
                ]),
                column("draft", "Draft", [
                    stage("state_2_proposal_drafted")
                ]),
                column("approval", "Approval", [
                    stage("state_3_initial_proposal_approval")
                ]),
                column("proposal-review-loop", "Proposal review loop", [
                    stage("state_4_proposal_reviewed", heightUnits: 2)
                ]),
                column("unique-targets", "Unique targets", [
                    stage("state_6_implementation_approval"),
                    stage("state_5_proposal_refined")
                ]),
                column("implementation-start", "Implementation start", [
                    stage("state_7_implementation_started", heightUnits: 2)
                ]),
                column("implementation-work-loop", "Implementation work loop", [
                    stage("state_8_implementation_continued"),
                    .bridge(id: "bridge-implementation-review-gate", label: "Review gate")
                ]),
                column("review-gate", "Review gate", [
                    stage("state_9_implementation_reviewed", heightUnits: 2)
                ]),
                column("closeout-branch", "Closeout branch", [
                    stage("state_10_implementation_refined"),
                    stage("state_11_manual_release")
                ]),
                column("terminal", "Terminal", [
                    stage("state_12_workflow_complete")
                ])
            ]

            let connectors = [
                connector("intake", [.primary]),
                connector("draft", [.primary]),
                connector("approval", [.primary]),
                connector("proposal-review-loop", [.primary, .retry]),
                connector("unique-targets", [.primary, .hidden]),
                connector("implementation-start", [.primary, .hidden]),
                connector("implementation-work-loop", [.primary, .hidden]),
                connector("review-gate", [.retry, .manual]),
                connector("closeout-branch", [.hidden, .primary])
            ]

            return Layout(columns: columns, connectors: connectors)
        }

        private static func sequentialLayout(for stages: [StageCard]) -> Layout {
            let columns = stages.map { stage in
                StageTopologyColumn(
                    id: "column-\(stage.id)",
                    title: "Stage \(stage.ordinal)",
                    slots: [.stage(stage)]
                )
            }
            let connectors = stages.dropLast().map { stage in
                StageTopologyConnectorColumn(
                    id: "connector-\(stage.id)",
                    connectors: [
                        StageTopologyConnector(id: "connector-\(stage.id)-primary", style: .primary)
                    ]
                )
            }
            return Layout(columns: columns, connectors: connectors)
        }
    }

    struct ApprovalRow: Identifiable, Equatable {
        let id: String
        let title: String
        let body: String?
        let canApprove: Bool
        let canReject: Bool
        let approveDisabledReason: String?
        let rejectDisabledReason: String?
        let deferredState: P036DeferredState?
        let accessibilityLabel: String
        // PC-001: follow-up reference and diagnostic copy items from P031/P085 row.
        // Both are suppressed when deferredState == .redacted.
        let followUpID: String?
        let copyItems: [P031DiagnosticCopyItem]
    }

    struct ArtifactReportRow: Identifiable, Equatable {
        let id: String
        let title: String
        let payloadAvailability: ArtifactPayloadAvailability
        let presentation: P031ArtifactViewerPresentation
    }

    enum ArtifactPayloadAvailability: String, Codable, Equatable {
        case available
        case metadataOnly = "metadata_only"
        case deferred
        case generating
        case unavailable
        case unknown

        init(from p085: P085ArtifactAffordanceState.PayloadPresentation) {
            switch p085 {
            case .available(_): self = .available
            case .metadataOnly: self = .metadataOnly
            case .deferred: self = .deferred
            case .generating: self = .generating
            case .unavailable(_): self = .unavailable
            case .unknown(_): self = .unknown
            }
        }
    }

    struct RecoveryEvidenceRow: Identifiable, Equatable {
        let id: String
        let title: String
    }

    struct FreshnessHealth: Equatable {
        let freshness: String
        let daemonHealth: String
        let schedulerHealth: String?
        let mcpHubStatus: String
        let capabilitiesStatus: String
        let isSystemReady: Bool
        /// True when daemon projection has not yet arrived; readiness cannot be determined.
        /// Views must render an explicit deferred/unavailable state rather than inferring false.
        let isReadinessDeferred: Bool
    }

    struct ActiveTimelineAgent: Identifiable, Equatable, Sendable {
        let id: String
        let title: String
        let providerID: String?
        let stageID: String?
        let stageLabel: String?
        let taskLabel: String?
        let status: String
        let sessionID: String?
        let latestAt: Date
        let eventCount: Int
        let selectionOrder: Int?
        let selectionUnavailableReason: String?
    }

    struct TimelineEntry: Identifiable, Equatable {
        let id: String
        let kind: TimelineEntryKind
        let title: String
        let detail: String
        let timestamp: Date
        let displayTime: String?
        let stageID: String?
        let surfaceLabel: String
        let agentID: String?
        let sessionID: String?
        let isCollapsed: Bool
        let rawDetail: String?
        let rawDetailBytes: Int?
        let rawDetailTruncated: Bool
        let rawDetailHandle: String?
        let rawDetailDigest: String?
        let fullRawAvailable: Bool
        let detailDigest: String?
        let detailCharCount: Int?
        let chunkCount: Int?
        let isStreaming: Bool
        let isTerminal: Bool
        let stateLabel: String?
        let providerID: String?

        init(
            id: String,
            kind: TimelineEntryKind,
            title: String,
            detail: String,
            timestamp: Date,
            displayTime: String?,
            stageID: String?,
            surfaceLabel: String,
            agentID: String?,
            sessionID: String?,
            isCollapsed: Bool,
            rawDetail: String? = nil,
            rawDetailBytes: Int? = nil,
            rawDetailTruncated: Bool = false,
            rawDetailHandle: String? = nil,
            rawDetailDigest: String? = nil,
            fullRawAvailable: Bool = true,
            detailDigest: String? = nil,
            detailCharCount: Int? = nil,
            chunkCount: Int? = nil,
            isStreaming: Bool = false,
            isTerminal: Bool = false,
            stateLabel: String? = nil,
            providerID: String? = nil
        ) {
            self.id = id
            self.kind = kind
            self.title = title
            self.detail = detail
            self.timestamp = timestamp
            self.displayTime = displayTime
            self.stageID = stageID
            self.surfaceLabel = surfaceLabel
            self.agentID = agentID
            self.sessionID = sessionID
            self.isCollapsed = isCollapsed
            self.rawDetail = rawDetail
            self.rawDetailBytes = rawDetailBytes
            self.rawDetailTruncated = rawDetailTruncated
            self.rawDetailHandle = rawDetailHandle
            self.rawDetailDigest = rawDetailDigest
            self.fullRawAvailable = fullRawAvailable
            self.detailDigest = detailDigest
            self.detailCharCount = detailCharCount
            self.chunkCount = chunkCount
            self.isStreaming = isStreaming
            self.isTerminal = isTerminal
            self.stateLabel = stateLabel
            self.providerID = providerID
        }
    }

    enum TimelineEntryKind: String, Codable, CaseIterable {
        case text
        case mergedTool = "merged_tool"
        case sessionEvent = "session_event"
        case agentSummary = "agent_summary"
        case policyWarning = "policy_warning"
        case implementationCompletion = "implementation_completion"
        case persisted
    }

    struct DeferredStateRow: Identifiable, Equatable {
        let id: String
        let summary: String
        let state: P036DeferredState
    }
}
