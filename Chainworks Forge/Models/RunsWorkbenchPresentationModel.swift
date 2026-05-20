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
            stages: detail.stageTransitions.map { transition in
                let status: String = {
                    switch transition.connectorState {
                    case .completed: return "terminal"
                    case .running: return "active"
                    case .blocked: return "blocked"
                    case .pending: return "pending"
                    case .unavailable: return "unavailable"
                    }
                }()
                return StageCard(
                    id: transition.stageExecutionID,
                    title: transition.stageTitle,
                    status: status,
                    attemptText: transition.attemptText,
                    startedLabel: transition.startedLabel,
                    completedLabel: transition.completedLabel,
                    durationLabel: transition.durationLabel,
                    evidenceLabels: transition.evidenceLabels,
                    artifactCount: detail.artifactViewerRows.filter { $0.stageExecutionID == transition.stageExecutionID }.count
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
            P036UICounters.shared.recordArtifactPayloadState(
                count: 1,
                payloadAvailabilityState: availability.rawValue,
                renderKind: "workbench_row"
            )
            return ArtifactReportRow(
                id: row.artifactID,
                title: row.title,
                payloadAvailability: availability,
                presentation: row
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
        
        // timelineEntries is populated from projection via populate(from: WorkflowMapProjection)
        
        if let error = detail.errorDescription {
            deferredStates = [
                DeferredStateRow(id: "error", summary: error, state: .unavailable)
            ]
        } else {
            deferredStates = []
        }
    }

    func populate(from projection: WorkflowMapProjection) {
        timelineEntries = buildFocusedTimelineSpineEntries(
            liveTimeline: projection.liveTimeline,
            persistedTimeline: projection.persistedTimeline,
            xcodeRuntimeObservations: projection.xcodeRuntimeObservations
        ).map { entry in
            TimelineEntry(
                id: entry.id,
                kind: TimelineEntryKind(rawValue: entry.kind.rawValue) ?? .text,
                message: entry.title + ": " + entry.detail,
                timestamp: entry.timestamp,
                agentID: entry.agentID,
                sessionID: entry.sessionID,
                isCollapsed: entry.isCollapsed
            )
        }
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

    // MARK: - Supporting Types
    struct SidebarLane: Identifiable, Equatable {
        let id: String
        let title: String
        let runs: [P031RunsHomeRowPresentation]
    }

    struct SummaryHeader: Equatable {
        let title: String
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
    }

    struct StageCard: Identifiable, Equatable {
        let id: String
        let title: String
        let status: String
        let attemptText: String?
        let startedLabel: String?
        let completedLabel: String?
        let durationLabel: String?
        let evidenceLabels: [String]
        let artifactCount: Int
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

    struct TimelineEntry: Identifiable, Equatable {
        let id: String
        let kind: TimelineEntryKind
        let message: String
        let timestamp: Date
        let agentID: String?
        let sessionID: String?
        let isCollapsed: Bool
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
