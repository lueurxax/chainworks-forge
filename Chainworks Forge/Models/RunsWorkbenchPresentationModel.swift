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
    @Published private(set) var recoveryEvidence: [RecoveryEvidenceRow] = []
    @Published private(set) var freshnessAndHealth: FreshnessHealth?
    @Published private(set) var timelineEntries: [TimelineEntry] = []
    @Published private(set) var deferredStates: [DeferredStateRow] = []

    init() {}

    // MARK: - Integration
    func populate(from runsHome: P031RunsHomePresentation) {
        var waiting: [P031RunsHomeRowPresentation] = []
        var blocked: [P031RunsHomeRowPresentation] = []
        var running: [P031RunsHomeRowPresentation] = []
        var completed: [P031RunsHomeRowPresentation] = []
        
        for row in runsHome.rows {
            let status = row.statusLabel.lowercased()
            if row.pendingApprovalsLabel != nil {
                waiting.append(row)
            } else if status.contains("blocked") || status.contains("failed") {
                blocked.append(row)
            } else if status.contains("completed") || status.contains("cancelled") {
                completed.append(row)
            } else {
                running.append(row)
            }
        }
        
        sidebarLanes = [
            SidebarLane(id: "waiting", title: "Waiting approval", runs: waiting),
            SidebarLane(id: "blocked", title: "Blocked or failed", runs: blocked),
            SidebarLane(id: "running", title: "Running", runs: running),
            SidebarLane(id: "completed", title: "Recently completed", runs: completed)
        ].filter { !$0.runs.isEmpty }
    }

    func populate(from detail: P031RunDetailPresentation) {
        summaryHeader = SummaryHeader(
            title: detail.title,
            status: detail.statusLabel
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
                    status: status
                )
            }
        )
        
        inlineApprovals = detail.approvalRows.map { row in
            let deferred: P036DeferredState? = {
                switch row.freshnessState {
                case .projectionLag: return .projectionLag
                case .stale: return .stale
                case .unauthorized: return .unauthorized
                case .unavailable: return .unavailable
                default: break
                }
                return nil
            }()
            
            return ApprovalRow(
                id: row.approvalID,
                title: row.title,
                canApprove: row.canApprove,
                canReject: row.canReject,
                approveDisabledReason: row.canApprove ? nil : row.actionLabel,
                rejectDisabledReason: row.canReject ? nil : row.actionLabel,
                deferredState: deferred
            )
        }
        
        artifactsAndReports = detail.artifactRows.map { row in
            let availability: ArtifactPayloadAvailability = row.canOpenPayload ? .available : .metadataOnly
            return ArtifactReportRow(
                id: row.artifactID,
                title: row.title,
                payloadAvailability: availability
            )
        }
        
        recoveryEvidence = [] // Future: map from detail when available
        
        timelineEntries = [] // Managed by TimelinePresentationModel
        
        deferredStates = [] // Future: map from detail when available
    }

    func updateFreshness(daemon: DaemonStatus?, scheduler: SchedulerHealthReadback?) {
        let daemonLabel = daemon?.state.rawValue ?? "disconnected"
        let schedulerLabel = scheduler?.health?.sustainedBackpressureState
        let hasBackpressure = scheduler?.health?.sustainedBackpressureState != "none"
        let isReady = daemon?.state == .ready && scheduler != nil && !hasBackpressure
        
        freshnessAndHealth = FreshnessHealth(
            freshness: freshnessAndHealth?.freshness ?? "unknown",
            daemonHealth: daemonLabel,
            schedulerHealth: schedulerLabel,
            isSystemReady: isReady
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
    }

    struct StageMap: Equatable {
        let stages: [StageCard]
    }

    struct StageCard: Identifiable, Equatable {
        let id: String
        let title: String
        let status: String
    }

    struct ApprovalRow: Identifiable, Equatable {
        let id: String
        let title: String
        let canApprove: Bool
        let canReject: Bool
        let approveDisabledReason: String?
        let rejectDisabledReason: String?
        let deferredState: P036DeferredState?
    }

    struct ArtifactReportRow: Identifiable, Equatable {
        let id: String
        let title: String
        let payloadAvailability: ArtifactPayloadAvailability
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
        let isSystemReady: Bool
    }

    struct TimelineEntry: Identifiable, Equatable {
        let id: String
        let kind: TimelineEntryKind
        let message: String
        let timestamp: Date
        let agentID: String?
        let isCollapsed: Bool
    }

    enum TimelineEntryKind: String, Codable, CaseIterable {
        case text
        case mergedTool = "merged_tool"
        case sessionEvent = "session_event"
        case agentSummary = "agent_summary"
    }

    struct DeferredStateRow: Identifiable, Equatable {
        let id: String
        let summary: String
        let state: P036DeferredState
    }
}
