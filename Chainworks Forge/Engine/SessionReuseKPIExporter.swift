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
        let mcpTelemetry: MCPTelemetrySummary
        let strategyTelemetry: StrategyTelemetrySummary
    }

    struct MCPTelemetrySummary: Codable, Sendable {
        struct StartupLatencyBucket: Codable, Sendable, Equatable {
            let extensionSet: String
            let executionCount: Int
            let totalStartupLatencyMilliseconds: Int64
            let averageStartupLatencyMilliseconds: Double
        }

        struct ServerUsageSummary: Codable, Sendable, Equatable {
            let serverID: String
            let toolCallCount: Int
            let requestBytes: Int64
            let responseBytes: Int64
            let promptContextDeltaBytes: Int64
        }

        let totalExecutionsWithMCPProfile: Int
        let totalZeroMCPExecutions: Int
        let totalRequestedExtensionCount: Int
        let totalPredictedExtensionCount: Int
        let totalActualExtensionCount: Int
        let totalDeniedExtensionCount: Int
        let totalPolicyReductionExecutions: Int
        let totalPredictionDriftExecutions: Int
        let averageRequestedExtensionsPerExecution: Double
        let averageActualExtensionsPerExecution: Double
        let totalStartupLatencyMilliseconds: Int64
        let averageStartupLatencyMilliseconds: Double
        let startupLatencyByExtensionSet: [StartupLatencyBucket]
        let serverUsage: [ServerUsageSummary]
        let totalPromptContextDeltaBytes: Int64
        let totalMCPPreflightBlockedRuns: Int
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
        let allExecutions = (run?.stageExecutions ?? []).flatMap(\.agentExecutions)
        let frozenMCPPolicies: [String: MCPPolicyResolutionReport] = {
            guard
                let data = run?.resolvedMCPPoliciesJSON,
                let decoded = try? JSONDecoder().decode([String: MCPPolicyResolutionReport].self, from: data)
            else {
                return [:]
            }
            return decoded
        }()

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
        let mcpExecutions = allExecutions.filter {
            guard let profileID = $0.mcpProfileID?.trimmingCharacters(in: .whitespacesAndNewlines) else { return false }
            return !profileID.isEmpty && profileID != "none"
        }
        let totalRequestedExtensionCount = mcpExecutions.reduce(0) { partial, execution in
            partial + decodeStringArray(execution.requestedMCPExtensionsJSON).count
        }
        let totalPredictedExtensionCount = mcpExecutions.reduce(0) { partial, execution in
            partial + (frozenMCPPolicies[execution.agentID]?.predictedEffectiveRuntimeExtensionIDs.count ?? 0)
        }
        let totalActualExtensionCount = mcpExecutions.reduce(0) { partial, execution in
            partial + decodeStringArray(execution.effectiveMCPRuntimeExtensionIDsJSON).count
        }
        let totalDeniedExtensionCount = mcpExecutions.reduce(0) { partial, execution in
            partial + decodeStringArray(execution.deniedMCPExtensionsJSON).count
        }
        let totalZeroMCPExecutions = mcpExecutions.filter {
            decodeStringArray($0.effectiveMCPRuntimeExtensionIDsJSON).isEmpty
        }.count
        let totalPolicyReductionExecutions = mcpExecutions.filter { execution in
            let requested = decodeStringArray(execution.requestedMCPExtensionsJSON)
            let actual = decodeStringArray(execution.effectiveMCPRuntimeExtensionIDsJSON)
            let denied = decodeStringArray(execution.deniedMCPExtensionsJSON)
            return actual.count < requested.count || !denied.isEmpty
        }.count
        let totalPredictionDriftExecutions = mcpExecutions.filter { execution in
            let predicted = Set(frozenMCPPolicies[execution.agentID]?.predictedEffectiveRuntimeExtensionIDs ?? [])
            let actual = Set(decodeStringArray(execution.effectiveMCPRuntimeExtensionIDsJSON))
            return predicted != actual
        }.count
        let startupLatencyValues = mcpExecutions.compactMap(\.mcpSessionStartupLatencyMilliseconds)
        let totalStartupLatencyMilliseconds = startupLatencyValues.reduce(Int64(0)) { $0 + Int64($1) }
        let averageStartupLatencyMilliseconds = startupLatencyValues.isEmpty
            ? 0.0
            : Double(totalStartupLatencyMilliseconds) / Double(startupLatencyValues.count)
        let startupLatencyByExtensionSet = startupLatencyBuckets(for: mcpExecutions)
        let serverUsage = aggregateServerUsage(from: mcpExecutions)
        let totalPromptContextDeltaBytes = serverUsage.reduce(Int64(0)) { $0 + $1.promptContextDeltaBytes }
        let totalMCPPreflightBlockedRuns: Int = {
            guard let r = run, r.status == .blocked else { return 0 }
            let reason = (r.driftDetails ?? "").lowercased()
            return (reason.contains("mcp") || reason.contains("extension registry") || reason.contains("session-scoped mcp")) ? 1 : 0
        }()
        let averageRequestedExtensionsPerExecution = mcpExecutions.isEmpty
            ? 0.0
            : Double(totalRequestedExtensionCount) / Double(mcpExecutions.count)
        let averageActualExtensionsPerExecution = mcpExecutions.isEmpty
            ? 0.0
            : Double(totalActualExtensionCount) / Double(mcpExecutions.count)

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
            mcpTelemetry: MCPTelemetrySummary(
                totalExecutionsWithMCPProfile: mcpExecutions.count,
                totalZeroMCPExecutions: totalZeroMCPExecutions,
                totalRequestedExtensionCount: totalRequestedExtensionCount,
                totalPredictedExtensionCount: totalPredictedExtensionCount,
                totalActualExtensionCount: totalActualExtensionCount,
                totalDeniedExtensionCount: totalDeniedExtensionCount,
                totalPolicyReductionExecutions: totalPolicyReductionExecutions,
                totalPredictionDriftExecutions: totalPredictionDriftExecutions,
                averageRequestedExtensionsPerExecution: averageRequestedExtensionsPerExecution,
                averageActualExtensionsPerExecution: averageActualExtensionsPerExecution,
                totalStartupLatencyMilliseconds: totalStartupLatencyMilliseconds,
                averageStartupLatencyMilliseconds: averageStartupLatencyMilliseconds,
                startupLatencyByExtensionSet: startupLatencyByExtensionSet,
                serverUsage: serverUsage,
                totalPromptContextDeltaBytes: totalPromptContextDeltaBytes,
                totalMCPPreflightBlockedRuns: totalMCPPreflightBlockedRuns
            ),
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

    private static func decodeStringArray(_ data: Data?) -> [String] {
        guard let data, let decoded = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }
        return decoded
    }

    private static func decodeMCPServerMetrics(_ data: Data?) -> [MCPServerExecutionMetric] {
        guard let data, let decoded = try? JSONDecoder().decode([MCPServerExecutionMetric].self, from: data) else {
            return []
        }
        return decoded
    }

    private static func startupLatencyBuckets(for executions: [AgentExecution]) -> [MCPTelemetrySummary.StartupLatencyBucket] {
        var buckets: [String: (count: Int, total: Int64)] = [:]
        for execution in executions {
            guard let latency = execution.mcpSessionStartupLatencyMilliseconds else { continue }
            let extensionSetValues = decodeStringArray(execution.effectiveMCPRuntimeExtensionIDsJSON)
            let extensionSet = extensionSetValues.isEmpty ? "none" : extensionSetValues.sorted().joined(separator: ",")
            var current = buckets[extensionSet] ?? (count: 0, total: 0)
            current.count += 1
            current.total += Int64(latency)
            buckets[extensionSet] = current
        }

        return buckets.keys.sorted().map { extensionSet in
            let current = buckets[extensionSet] ?? (count: 0, total: 0)
            let average = current.count > 0 ? Double(current.total) / Double(current.count) : 0.0
            return MCPTelemetrySummary.StartupLatencyBucket(
                extensionSet: extensionSet,
                executionCount: current.count,
                totalStartupLatencyMilliseconds: current.total,
                averageStartupLatencyMilliseconds: average
            )
        }
    }

    private static func aggregateServerUsage(from executions: [AgentExecution]) -> [MCPTelemetrySummary.ServerUsageSummary] {
        var aggregate: [String: (count: Int, requestBytes: Int64, responseBytes: Int64, promptDeltaBytes: Int64)] = [:]
        for execution in executions {
            for metric in decodeMCPServerMetrics(execution.mcpServerTelemetryJSON) {
                var current = aggregate[metric.serverID] ?? (count: 0, requestBytes: 0, responseBytes: 0, promptDeltaBytes: 0)
                current.count += metric.toolCallCount
                current.requestBytes += metric.requestBytes
                current.responseBytes += metric.responseBytes
                current.promptDeltaBytes += metric.promptContextDeltaBytes
                aggregate[metric.serverID] = current
            }
        }

        return aggregate.keys.sorted().map { serverID in
            let current = aggregate[serverID] ?? (count: 0, requestBytes: 0, responseBytes: 0, promptDeltaBytes: 0)
            return MCPTelemetrySummary.ServerUsageSummary(
                serverID: serverID,
                toolCallCount: current.count,
                requestBytes: current.requestBytes,
                responseBytes: current.responseBytes,
                promptContextDeltaBytes: current.promptDeltaBytes
            )
        }
    }

    private static func mcpPreflightBlockedRunCount(for run: Run) -> Int {
        guard run.status == .blocked else { return 0 }
        let reason = (run.driftDetails ?? "").lowercased()
        if reason.contains("mcp")
            || reason.contains("extension registry")
            || reason.contains("session-scoped mcp")
            || reason.contains("unknown mcp profile") {
            return 1
        }
        return 0
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
