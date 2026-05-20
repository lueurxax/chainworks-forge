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
    
    // P036 UX Consolidation metrics
    let p036NavigationTabSelection: Int
    let p036WorkbenchLaneCount: Int
    let p036TimelineEntryCount: Int

    // P036 required operational metrics (event counters; zero until event-site instrumentation lands)
    let p036TabRouteResolutionTotal: Int
    let p036GlobalAttentionIndicatorTotal: Int
    let p036InlineApprovalRenderTotal: Int
    let p036OperatorTaskAttemptTotal: Int
    let p036TimelineBatchFlushTotal: Int
    let p036TimelineCardCollapseTotal: Int
    let p036ArtifactPayloadStateTotal: Int
    let p036ProjectionGapDeferredTotal: Int
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

        // P036 Metrics (ARCH-036) — derived from loopCounters where recorded
        let navSelections = allRuns.compactMap { $0.loopCounters["p036_navigation_tab_selection"] }.reduce(0, +)
        let laneCount = allRuns.compactMap { $0.loopCounters["p036_workbench_lane_count"] }.sorted().last ?? 0
        let timelineEntryCount = allRuns.compactMap { $0.loopCounters["p036_timeline_entry_count"] }.sorted().last ?? 0
        // Required named metrics (event-site counters; accumulated from loopCounters, zero until wired)
        let tabRouteResTotal = allRuns.compactMap { $0.loopCounters["p036_tab_route_resolution_total"] }.reduce(0, +)
        let attentionTotal = allRuns.compactMap { $0.loopCounters["p036_global_attention_indicator_total"] }.reduce(0, +)
        let approvalRenderTotal = allRuns.compactMap { $0.loopCounters["p036_inline_approval_render_total"] }.reduce(0, +)
        let taskAttemptTotal = allRuns.compactMap { $0.loopCounters["p036_operator_task_attempt_total"] }.reduce(0, +)
        let batchFlushTotal = allRuns.compactMap { $0.loopCounters["p036_timeline_batch_flush_total"] }.reduce(0, +)
        let cardCollapseTotal = allRuns.compactMap { $0.loopCounters["p036_timeline_card_collapse_total"] }.reduce(0, +)
        let artifactPayloadTotal = allRuns.compactMap { $0.loopCounters["p036_artifact_payload_state_total"] }.reduce(0, +)
        let projectionGapTotal = allRuns.compactMap { $0.loopCounters["p036_projection_gap_deferred_total"] }.reduce(0, +)

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
            resumedRunCount: resumedRuns.count,
            p036NavigationTabSelection: navSelections,
            p036WorkbenchLaneCount: laneCount,
            p036TimelineEntryCount: timelineEntryCount,
            p036TabRouteResolutionTotal: tabRouteResTotal,
            p036GlobalAttentionIndicatorTotal: attentionTotal,
            p036InlineApprovalRenderTotal: approvalRenderTotal,
            p036OperatorTaskAttemptTotal: taskAttemptTotal,
            p036TimelineBatchFlushTotal: batchFlushTotal,
            p036TimelineCardCollapseTotal: cardCollapseTotal,
            p036ArtifactPayloadStateTotal: artifactPayloadTotal,
            p036ProjectionGapDeferredTotal: projectionGapTotal
        )
    }

    private func emptySnapshot() -> MetricsSnapshot {
        MetricsSnapshot(
            runCount: 0, windowStart: Date(), windowEnd: Date(),
            leadTimeMedianSeconds: nil, stageLatencyMedians: [:], approvalWaitMedianSeconds: nil,
            proposalLoopMean: 0, implementationLoopMean: 0, retriesPerStageMean: [:],
            approvalRejectionRate: 0, auditPassRate: 1.0,
            costPerRunMedianCents: nil, costByStageFamily: [:],
            failedRunRate: 0, blockedRunRate: 0, driftEventCount: 0, resumedRunCount: 0,
            p036NavigationTabSelection: 0, p036WorkbenchLaneCount: 0, p036TimelineEntryCount: 0,
            p036TabRouteResolutionTotal: 0, p036GlobalAttentionIndicatorTotal: 0,
            p036InlineApprovalRenderTotal: 0, p036OperatorTaskAttemptTotal: 0,
            p036TimelineBatchFlushTotal: 0, p036TimelineCardCollapseTotal: 0,
            p036ArtifactPayloadStateTotal: 0, p036ProjectionGapDeferredTotal: 0
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
