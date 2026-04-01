import Foundation
import SwiftData

// MARK: - SessionReuseKPIExporter (Proposal 018, §8)

/// Exports the section 8 minimum KPIs for measuring session reuse burn reduction.
///
/// KPIs:
/// - percent of executions using a reused session
/// - cold_start_tokens_saved
/// - average input tokens per invocation by agent
/// - session_growth_tokens
/// - forced resets due to budget
/// - token savings versus fresh baseline
final class SessionReuseKPIExporter {

    /// Per-agent KPI snapshot.
    struct AgentKPI: Codable, Sendable {
        let agentID: String
        let totalExecutions: Int
        let reusedExecutions: Int
        let freshExecutions: Int
        let reusePercentage: Double
        let coldStartTokensSaved: Int64
        let averageInputTokensPerInvocation: Int64
        let sessionGrowthTokens: Int64
        let forcedBudgetResets: Int
        let tokenSavingsVersusFreshBaseline: Int64
    }

    /// Aggregate KPI summary for the entire run.
    struct RunKPISummary: Codable, Sendable {
        let runID: UUID
        let exportedAt: Date
        let totalExecutions: Int
        let totalReusedExecutions: Int
        let overallReusePercentage: Double
        let totalColdStartTokensSaved: Int64
        let totalSessionGrowthTokens: Int64
        let totalForcedBudgetResets: Int
        let totalTokenSavingsVersusFreshBaseline: Int64
        let perAgentKPIs: [AgentKPI]
        let strategyTelemetry: StrategyTelemetrySummary
    }

    struct StrategyTelemetrySummary: Codable, Sendable {
        let totalPayloadBytesBeforeStrategy: Int64
        let totalPayloadBytesAfterStrategy: Int64
        let totalPayloadReductionBytes: Int64
        let averageLazyArtifactCount: Double
        let totalLazyEvidenceHitCount: Int
        let averageLazyEvidenceHitRate: Double
        let averageCacheEffectiveness: Double
        let totalCompactionChurn: Int
        let totalEscalationCount: Int
        let totalRetryableEscalationCount: Int
        let totalContractFailureCount: Int
        let operatorPromotedArtifactCount: Int
        let totalPromotedArtifactUsages: Int
    }

    static func decodeSummary(from data: Data?) -> RunKPISummary? {
        guard let data else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(RunKPISummary.self, from: data)
    }

    static func hasCanonicalStrategyTelemetry(_ summary: RunKPISummary?) -> Bool {
        guard let summary, summary.totalExecutions > 0 else { return false }
        let telemetry = summary.strategyTelemetry
        return telemetry.totalPayloadBytesBeforeStrategy >= 0
            && telemetry.totalPayloadBytesAfterStrategy >= 0
            && telemetry.totalPayloadReductionBytes >= 0
            && telemetry.averageLazyArtifactCount >= 0
            && telemetry.totalLazyEvidenceHitCount >= 0
            && telemetry.averageLazyEvidenceHitRate >= 0
            && telemetry.averageCacheEffectiveness >= 0
            && telemetry.totalCompactionChurn >= 0
            && telemetry.totalEscalationCount >= 0
            && telemetry.totalRetryableEscalationCount >= 0
            && telemetry.totalContractFailureCount >= 0
            && telemetry.operatorPromotedArtifactCount >= 0
            && telemetry.totalPromotedArtifactUsages >= 0
    }

    /// Generate KPI summary for a run.
    static func exportKPIs(for runID: UUID, context: ModelContext) -> RunKPISummary {
        let runDescriptor = FetchDescriptor<Run>(
            predicate: #Predicate<Run> { $0.id == runID }
        )
        let run = try? context.fetch(runDescriptor).first

        // Fetch all session lineages for this run
        let lineagePredicate = #Predicate<AgentSessionLineage> { $0.runID == runID }
        let lineageDescriptor = FetchDescriptor<AgentSessionLineage>(predicate: lineagePredicate)
        let lineages = (try? context.fetch(lineageDescriptor)) ?? []

        // Build per-agent KPIs from lineage + generation data
        var agentKPIs: [AgentKPI] = []
        var totalExecutions = 0
        var totalReused = 0
        var totalColdStartSaved: Int64 = 0
        var totalGrowth: Int64 = 0
        var totalBudgetResets = 0
        var totalSavings: Int64 = 0

        let cacheShares = lineages.flatMap { lineage in
            lineage.generations.compactMap { generation -> Double? in
                guard generation.estimatedInputTokens > 0, generation.turnCount > 0 else { return nil }
                let averageTurnTokens = generation.cumulativePromptTokens / Int64(max(1, generation.turnCount))
                let staticPrefix = max(Int64(0), generation.estimatedInputTokens - averageTurnTokens)
                return Double(staticPrefix) / Double(max(Int64(1), generation.estimatedInputTokens))
            }
        }
        let lineageCompactionChurn = lineages.reduce(into: 0) { partial, lineage in
            partial += lineage.events.filter { $0.eventType == .compacted }.count
        }

        let strategySignals = (run?.stageExecutions ?? [])
            .flatMap(\.agentExecutions)
            .compactMap { execution -> StrategyLimitPressureSignals? in
                guard let data = execution.limitPressureSignalsJSON else { return nil }
                return try? JSONDecoder().decode(StrategyLimitPressureSignals.self, from: data)
            }

        for lineage in lineages {
            let generations = lineage.generations.sorted(by: { $0.generation < $1.generation })
            let events = lineage.events

            // Count executions by disposition
            let reuseEvents = events.filter { $0.eventType == .reused || $0.eventType == .resume_reused }
            let createEvents = events.filter { $0.eventType == .created }
            let budgetEvents = events.filter { $0.eventType == .budget_exceeded }

            let agentTotalExec = reuseEvents.count + createEvents.count
            let agentReused = reuseEvents.count
            let agentFresh = createEvents.count

            // Token economics
            let totalTurns = generations.reduce(0) { $0 + $1.turnCount }
            let totalTokens = generations.reduce(Int64(0)) { $0 + $1.cumulativePromptTokens }
            let avgTokensPerInvocation = agentTotalExec > 0 ? totalTokens / Int64(max(1, agentTotalExec)) : Int64(0)

            // Cold start savings: estimate tokens saved by reusing vs. starting fresh each time.
            // Fresh baseline per turn = first generation's average prompt tokens per turn.
            let freshBaselinePerTurn: Int64
            if let firstGen = generations.first, firstGen.turnCount > 0 {
                freshBaselinePerTurn = firstGen.cumulativePromptTokens / Int64(firstGen.turnCount)
            } else {
                freshBaselinePerTurn = avgTokensPerInvocation
            }
            let coldStartSaved = Int64(agentReused) * freshBaselinePerTurn

            // Session growth: tokens above fresh baseline across all generations.
            let growthTokens = generations.reduce(Int64(0)) { total, gen in
                let baseline = freshBaselinePerTurn * Int64(gen.turnCount)
                let actual = gen.cumulativePromptTokens
                return total + max(0, actual - baseline)
            }

            // Net savings: cold start savings minus growth cost.
            let netSavings = coldStartSaved - growthTokens

            let reusePercentage = agentTotalExec > 0 ? Double(agentReused) / Double(agentTotalExec) * 100.0 : 0.0

            let kpi = AgentKPI(
                agentID: lineage.agentID,
                totalExecutions: agentTotalExec,
                reusedExecutions: agentReused,
                freshExecutions: agentFresh,
                reusePercentage: reusePercentage,
                coldStartTokensSaved: coldStartSaved,
                averageInputTokensPerInvocation: avgTokensPerInvocation,
                sessionGrowthTokens: growthTokens,
                forcedBudgetResets: budgetEvents.count,
                tokenSavingsVersusFreshBaseline: netSavings
            )
            agentKPIs.append(kpi)

            totalExecutions += agentTotalExec
            totalReused += agentReused
            totalColdStartSaved += coldStartSaved
            totalGrowth += growthTokens
            totalBudgetResets += budgetEvents.count
            totalSavings += netSavings
        }

        let overallReuse = totalExecutions > 0 ? Double(totalReused) / Double(totalExecutions) * 100.0 : 0.0
        let totalPayloadBytesBeforeStrategy = strategySignals.reduce(Int64(0)) { $0 + Int64($1.payloadBytesBeforeStrategy) }
        let totalPayloadBytesAfterStrategy = strategySignals.reduce(Int64(0)) { $0 + Int64($1.payloadBytesAfterStrategy) }
        let totalPayloadReductionBytes = strategySignals.reduce(Int64(0)) { $0 + Int64($1.payloadReductionBytes) }
        let averageLazyArtifactCount = strategySignals.isEmpty
            ? 0.0
            : Double(strategySignals.reduce(0) { $0 + $1.lazyArtifactCount }) / Double(strategySignals.count)
        let totalLazyEvidenceHitCount = strategySignals.reduce(0) { partial, signal in
            partial + (signal.lazyEvidenceHitCount ?? 0)
        }
        let lazyEvidenceHitRates = strategySignals.compactMap(\.lazyEvidenceHitRate)
        let averageLazyEvidenceHitRate = lazyEvidenceHitRates.isEmpty
            ? 0.0
            : lazyEvidenceHitRates.reduce(0.0, +) / Double(lazyEvidenceHitRates.count)
        let canonicalCacheShares = strategySignals.compactMap(\.cacheEffectiveness)
        let averageCacheEffectiveness = if !canonicalCacheShares.isEmpty {
            canonicalCacheShares.reduce(0.0, +) / Double(canonicalCacheShares.count)
        } else if !cacheShares.isEmpty {
            cacheShares.reduce(0.0, +) / Double(cacheShares.count)
        } else {
            0.0
        }
        let canonicalCompactionChurn = strategySignals.compactMap(\.compactionChurnCount)
        let totalCompactionChurn = if !canonicalCompactionChurn.isEmpty {
            canonicalCompactionChurn.reduce(0, +)
        } else {
            lineageCompactionChurn
        }
        let totalEscalationCount = strategySignals.reduce(0) { $0 + $1.escalationCount }
        let totalRetryableEscalationCount = strategySignals.reduce(0) { $0 + $1.retryableEscalationCount }
        let totalContractFailureCount = strategySignals.reduce(0) { $0 + $1.contractFailureCount }
        let totalPromotedArtifactUsages = strategySignals.reduce(0) { $0 + $1.operatorPromotedArtifactCount }
        let operatorPromotedArtifactCount: Int = {
            guard
                let data = run?.promotedHandoffArtifactsJSON,
                let artifacts = try? JSONDecoder().decode([String].self, from: data)
            else {
                return 0
            }
            return artifacts.count
        }()

        return RunKPISummary(
            runID: runID,
            exportedAt: Date(),
            totalExecutions: totalExecutions,
            totalReusedExecutions: totalReused,
            overallReusePercentage: overallReuse,
            totalColdStartTokensSaved: totalColdStartSaved,
            totalSessionGrowthTokens: totalGrowth,
            totalForcedBudgetResets: totalBudgetResets,
            totalTokenSavingsVersusFreshBaseline: totalSavings,
            perAgentKPIs: agentKPIs,
            strategyTelemetry: StrategyTelemetrySummary(
                totalPayloadBytesBeforeStrategy: totalPayloadBytesBeforeStrategy,
                totalPayloadBytesAfterStrategy: totalPayloadBytesAfterStrategy,
                totalPayloadReductionBytes: totalPayloadReductionBytes,
                averageLazyArtifactCount: averageLazyArtifactCount,
                totalLazyEvidenceHitCount: totalLazyEvidenceHitCount,
                averageLazyEvidenceHitRate: averageLazyEvidenceHitRate,
                averageCacheEffectiveness: averageCacheEffectiveness,
                totalCompactionChurn: totalCompactionChurn,
                totalEscalationCount: totalEscalationCount,
                totalRetryableEscalationCount: totalRetryableEscalationCount,
                totalContractFailureCount: totalContractFailureCount,
                operatorPromotedArtifactCount: operatorPromotedArtifactCount,
                totalPromotedArtifactUsages: totalPromotedArtifactUsages
            )
        )
    }

    /// Export KPIs as JSON for inclusion in run reports or external pipelines.
    static func exportJSON(for runID: UUID, context: ModelContext) -> Data? {
        let summary = exportKPIs(for: runID, context: context)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try? encoder.encode(summary)
    }
}
