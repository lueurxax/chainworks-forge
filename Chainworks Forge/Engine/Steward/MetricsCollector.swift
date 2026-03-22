import Foundation
import SwiftData

/// Deterministic metrics computed from persisted run data.
/// All values are derived from SwiftData relationships — no LLM involvement.
struct MetricsSnapshot: Codable, Hashable, Sendable {
    let runCount: Int
    let windowStart: Date
    let windowEnd: Date

    // Timing
    let leadTimeMedianSeconds: Double?
    let stageLatencyMedians: [String: Double]
    let approvalWaitMedianSeconds: Double?

    // Rework
    let proposalLoopMean: Double
    let implementationLoopMean: Double
    let retriesPerStageMean: [String: Double]

    // Quality
    let approvalRejectionRate: Double
    let auditPassRate: Double

    // Cost
    let costPerRunMedianCents: Int64?
    let costByStageFamily: [String: Int64]

    // Stability
    let failedRunRate: Double
    let blockedRunRate: Double
    let driftEventCount: Int
    let resumedRunCount: Int
}

/// Collects deterministic metrics from a set of completed runs.
@MainActor
struct MetricsCollector {
    let modelContext: ModelContext

    func collectMetrics(for runs: [Run]) -> MetricsSnapshot {
        guard !runs.isEmpty else {
            return emptySnapshot()
        }

        let completedRuns = runs.filter { $0.status == .completed }
        let allRuns = runs

        // Timing: lead time = completedAt - startedAt
        let leadTimes = completedRuns.compactMap { run -> Double? in
            guard let completed = run.completedAt else { return nil }
            return completed.timeIntervalSince(run.startedAt)
        }

        // Stage latencies: per stageID, median duration
        var stageDurations: [String: [Double]] = [:]
        for run in allRuns {
            for stage in run.stageExecutions {
                let duration = (stage.completedAt ?? Date()).timeIntervalSince(stage.startedAt)
                stageDurations[stage.stageID, default: []].append(duration)
            }
        }

        // Approval wait times
        let approvalWaits = allRuns.flatMap { $0.approvals }.compactMap { approval -> Double? in
            guard let decided = approval.decidedAt else { return nil }
            return decided.timeIntervalSince(approval.requestedAt)
        }

        // Rework: loop counters
        let proposalLoops = allRuns.compactMap { $0.loopCounters["proposal_refinement_loop"] }.map(Double.init)
        let implLoops = allRuns.compactMap { $0.loopCounters["implementation_refinement_loop"] }.map(Double.init)

        // Retries per stage
        var retriesByStage: [String: [Double]] = [:]
        for run in allRuns {
            for stage in run.stageExecutions {
                retriesByStage[stage.stageID, default: []].append(Double(stage.attemptNumber))
            }
        }

        // Quality
        let allApprovals = allRuns.flatMap { $0.approvals }
        let decidedApprovals = allApprovals.filter { $0.decision == .granted || $0.decision == .rejected }
        let rejections = allApprovals.filter { $0.decision == .rejected }
        let rejectionRate = decidedApprovals.isEmpty ? 0 : Double(rejections.count) / Double(decidedApprovals.count)

        // Audit pass rate
        let auditStages = allRuns.flatMap { $0.stageExecutions }.filter { $0.stageID.contains("audit") || $0.stageID.contains("review") }
        let auditCompleted = auditStages.filter { $0.status == .completed }
        let auditPassRate = auditStages.isEmpty ? 1.0 : Double(auditCompleted.count) / Double(auditStages.count)

        // Cost
        let costs = completedRuns.compactMap { $0.totalCostCents }
        var costByStage: [String: Int64] = [:]
        for run in allRuns {
            for stage in run.stageExecutions {
                let stageCost = stage.agentExecutions.compactMap(\.costCents).reduce(0, +)
                costByStage[stage.stageID, default: 0] += stageCost
            }
        }

        // Stability
        let failedRuns = allRuns.filter { $0.status == .failed }
        let blockedRuns = allRuns.filter { $0.status == .blocked }
        let driftEvents = allRuns.filter { $0.driftDetectedAt != nil }
        let resumedRuns = allRuns.filter { run in
            run.status == .running && run.stageExecutions.contains { $0.attemptNumber > 1 }
        }

        let sortedDates = allRuns.map(\.startedAt).sorted()

        return MetricsSnapshot(
            runCount: allRuns.count,
            windowStart: sortedDates.first ?? Date(),
            windowEnd: sortedDates.last ?? Date(),
            leadTimeMedianSeconds: median(leadTimes),
            stageLatencyMedians: stageDurations.mapValues { median($0) ?? 0 },
            approvalWaitMedianSeconds: median(approvalWaits),
            proposalLoopMean: mean(proposalLoops),
            implementationLoopMean: mean(implLoops),
            retriesPerStageMean: retriesByStage.mapValues { mean($0) },
            approvalRejectionRate: rejectionRate,
            auditPassRate: auditPassRate,
            costPerRunMedianCents: costs.isEmpty ? nil : Int64(median(costs.map(Double.init)) ?? 0),
            costByStageFamily: costByStage,
            failedRunRate: allRuns.isEmpty ? 0 : Double(failedRuns.count) / Double(allRuns.count),
            blockedRunRate: allRuns.isEmpty ? 0 : Double(blockedRuns.count) / Double(allRuns.count),
            driftEventCount: driftEvents.count,
            resumedRunCount: resumedRuns.count
        )
    }

    private func emptySnapshot() -> MetricsSnapshot {
        MetricsSnapshot(
            runCount: 0, windowStart: Date(), windowEnd: Date(),
            leadTimeMedianSeconds: nil, stageLatencyMedians: [:], approvalWaitMedianSeconds: nil,
            proposalLoopMean: 0, implementationLoopMean: 0, retriesPerStageMean: [:],
            approvalRejectionRate: 0, auditPassRate: 1.0,
            costPerRunMedianCents: nil, costByStageFamily: [:],
            failedRunRate: 0, blockedRunRate: 0, driftEventCount: 0, resumedRunCount: 0
        )
    }

    // MARK: - Statistics helpers

    private func median(_ values: [Double]) -> Double? {
        guard !values.isEmpty else { return nil }
        let sorted = values.sorted()
        let mid = sorted.count / 2
        if sorted.count.isMultiple(of: 2) {
            return (sorted[mid - 1] + sorted[mid]) / 2
        }
        return sorted[mid]
    }

    private func mean(_ values: [Double]) -> Double {
        guard !values.isEmpty else { return 0 }
        return values.reduce(0, +) / Double(values.count)
    }
}
