import Foundation
import SwiftData

// MARK: - Dossier types

struct RunDossier: Codable, Sendable {
    let runID: UUID
    let startedAt: Date
    let completedAt: Date?
    let status: String
    let workflowSnapshotHash: String
    let catalogSnapshotHash: String
    let stageExecutionSummaries: [StageExecutionSummary]
    let approvalHistory: [ApprovalSummary]
    let costBreakdown: CostBreakdown
    let failureRetryEvents: [FailureEvent]
    let artifactManifest: [ArtifactSummary]
    let loopCounters: [String: Int]
    let driftDetectedAt: Date?
    let driftDetails: String?
}

struct StageExecutionSummary: Codable, Sendable {
    let stageID: String
    let label: String
    let status: String
    let durationSeconds: Double
    let iteration: Int
    let attemptNumber: Int
    let agentCount: Int
}

struct ApprovalSummary: Codable, Sendable {
    let stageID: String
    let decision: String
    let waitSeconds: Double?
    let comment: String?
}

struct CostBreakdown: Codable, Sendable {
    let totalCostCents: Int64
    let costByStage: [String: Int64]
    let costByAgent: [String: Int64]
}

struct FailureEvent: Codable, Sendable {
    let stageID: String
    let agentID: String?
    let status: String
    let attemptNumber: Int
    let retryReason: String?
}

struct ArtifactSummary: Codable, Sendable {
    let name: String
    let format: String
    let sizeBytes: Int64?
    let agentID: String
    let stageID: String
}

// MARK: - Builder

/// Builds evidence dossiers for implicated runs.
/// All data is extracted deterministically from SwiftData relationships.
@MainActor
struct RunDossierBuilder {
    let modelContext: ModelContext

    func buildDossier(for run: Run) -> RunDossier {
        let stages = run.stageExecutions.sorted { $0.startedAt < $1.startedAt }

        let stageSummaries = stages.map { stage -> StageExecutionSummary in
            let duration = (stage.completedAt ?? Date()).timeIntervalSince(stage.startedAt)
            return StageExecutionSummary(
                stageID: stage.stageID,
                label: stage.label,
                status: stage.status.rawValue,
                durationSeconds: duration,
                iteration: stage.iteration,
                attemptNumber: stage.attemptNumber,
                agentCount: stage.agentExecutions.count
            )
        }

        let approvals = run.approvals.sorted { $0.requestedAt < $1.requestedAt }.map { approval -> ApprovalSummary in
            let wait: Double? = approval.decidedAt.map { $0.timeIntervalSince(approval.requestedAt) }
            return ApprovalSummary(
                stageID: approval.stageID,
                decision: approval.decision.rawValue,
                waitSeconds: wait,
                comment: approval.comment
            )
        }

        var costByStage: [String: Int64] = [:]
        var costByAgent: [String: Int64] = [:]
        var failureEvents: [FailureEvent] = []
        var artifacts: [ArtifactSummary] = []

        for stage in stages {
            var stageCost: Int64 = 0
            for agent in stage.agentExecutions {
                let cost = agent.costCents ?? 0
                stageCost += cost
                costByAgent[agent.agentID, default: 0] += cost

                if agent.status == .failed || agent.status == .cancelled {
                    failureEvents.append(FailureEvent(
                        stageID: stage.stageID,
                        agentID: agent.agentID,
                        status: agent.status.rawValue,
                        attemptNumber: stage.attemptNumber,
                        retryReason: agent.retryReason
                    ))
                }

                for artifact in agent.artifacts {
                    artifacts.append(ArtifactSummary(
                        name: artifact.name,
                        format: artifact.format.rawValue,
                        sizeBytes: artifact.sizeBytes,
                        agentID: artifact.agentID,
                        stageID: artifact.stageID
                    ))
                }
            }
            costByStage[stage.stageID] = stageCost
        }

        for stage in stages where stage.status == .failed || stage.status == .blocked {
            if !failureEvents.contains(where: { $0.stageID == stage.stageID }) {
                failureEvents.append(FailureEvent(
                    stageID: stage.stageID,
                    agentID: nil,
                    status: stage.status.rawValue,
                    attemptNumber: stage.attemptNumber,
                    retryReason: nil
                ))
            }
        }

        return RunDossier(
            runID: run.id,
            startedAt: run.startedAt,
            completedAt: run.completedAt,
            status: run.status.rawValue,
            workflowSnapshotHash: run.workflowSnapshotHash,
            catalogSnapshotHash: run.catalogSnapshotHash,
            stageExecutionSummaries: stageSummaries,
            approvalHistory: approvals,
            costBreakdown: CostBreakdown(
                totalCostCents: run.totalCostCents ?? 0,
                costByStage: costByStage,
                costByAgent: costByAgent
            ),
            failureRetryEvents: failureEvents,
            artifactManifest: artifacts,
            loopCounters: run.loopCounters,
            driftDetectedAt: run.driftDetectedAt,
            driftDetails: run.driftDetails
        )
    }

    func buildDossiers(for runs: [Run]) -> [RunDossier] {
        runs.map { buildDossier(for: $0) }
    }
}
