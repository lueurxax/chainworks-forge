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
            if row.pendingApprovalsLabel != nil {
                waiting.append(row)
            } else {
                let status = row.statusLabel.lowercased()
                if status.contains("blocked") || status.contains("failed") || status.contains("error") {
                    blocked.append(row)
                } else if status.contains("running") || status.contains("active") || status.contains("started") {
                    running.append(row)
                } else {
                    completed.append(row)
                }
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
                StageCard(
                    id: transition.stageExecutionID,
                    title: transition.stageTitle,
                    status: transition.connectorState == .completed ? "terminal" : "active"
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
                default: return nil
                }
            }()
            
            return ApprovalRow(
                id: row.approvalID,
                title: row.title,
                isActionable: (row.canApprove || row.canReject) && deferred == nil,
                disabledReason: deferred != nil ? "Stale or unauthorized" : ((row.canApprove || row.canReject) ? nil : "Already resolved or unavailable"),
                deferredState: deferred
            )
        }
        
        artifactsAndReports = detail.artifactRows.map { row in
            ArtifactReportRow(
                id: row.artifactID,
                title: row.title,
                payloadAvailability: "available" // Simplified for now
            )
        }
        
        freshnessAndHealth = FreshnessHealth(
            freshness: detail.freshness.state.rawValue,
            daemonHealth: "Healthy" // Placeholder
        )
        
        timelineEntries = [] // Will be populated from separate timeline source
        
        if let error = detail.errorDescription {
            deferredStates = [
                DeferredStateRow(id: "error", summary: error, state: .unavailable)
            ]
        } else {
            deferredStates = []
        }
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
        let isActionable: Bool
        let disabledReason: String?
        let deferredState: P036DeferredState?
    }

    struct ArtifactReportRow: Identifiable, Equatable {
        let id: String
        let title: String
        let payloadAvailability: String
    }

    struct RecoveryEvidenceRow: Identifiable, Equatable {
        let id: String
        let title: String
    }

    struct FreshnessHealth: Equatable {
        let freshness: String
        let daemonHealth: String
    }

    struct TimelineEntry: Identifiable, Equatable {
        let id: String
        let message: String
    }

    struct DeferredStateRow: Identifiable, Equatable {
        let id: String
        let summary: String
        let state: P036DeferredState
    }
}
