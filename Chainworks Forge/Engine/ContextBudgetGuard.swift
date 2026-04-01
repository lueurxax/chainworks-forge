import Foundation

enum BudgetDecision {
    case continueReuse
    case compact(reason: String)
    case invalidate(reason: String)
}

/// Budget-driven invalidation and compaction guard for session reuse (§6.3).
///
/// Reuse is not automatically a savings win. This guard evaluates whether
/// continued reuse is economically justified by checking both hard guardrails
/// and measured reuse-economics signals.
///
/// The policy result is one of:
/// - continue reuse
/// - create checkpoint and compact into a fresh generation
/// - invalidate into `fresh_after_budget`
final class ContextBudgetGuard {

    /// Numerical guardrails (§6.3, subject to per-provider refinement).
    struct BudgetConfig {
        let maxTurns: Int
        let maxEstimatedInputTokens: Int64
        let maxCumulativePromptTokens: Int64
        let maxCumulativeCostCents: Int64
        let maxIdleAgeSeconds: TimeInterval
        let maxTranscriptGrowthRatio: Double
        /// Minimum cache-hit share below which reuse loses its advantage.
        let minCachedTokenShare: Double
        /// Cost penalty threshold (cents) above which fresh-baseline is strictly better.
        let maxReuseCostPenaltyCents: Double
        /// Effective prompt size threshold (fraction of model context window).
        let maxEffectivePromptSizeFraction: Double

        static let `default` = BudgetConfig(
            maxTurns: 20,
            maxEstimatedInputTokens: 128_000,
            maxCumulativePromptTokens: 1_000_000,
            maxCumulativeCostCents: 500,
            maxIdleAgeSeconds: 4 * 3600,
            maxTranscriptGrowthRatio: 2.0,
            minCachedTokenShare: 0.2,
            maxReuseCostPenaltyCents: 5.0,
            maxEffectivePromptSizeFraction: 0.5
        )
    }

    /// Measured reuse-economics signals (§6.3).
    ///
    /// These are the primary decision authority — guardrails are backstops,
    /// not the primary decision driver.
    struct EconomicSignals {
        /// Fraction of input tokens that are cached/reused by the provider (0.0–1.0).
        let cachedTokenShare: Double?
        /// Net savings of reuse vs. fresh baseline (positive = reuse cheaper, negative = reuse more expensive).
        let normalizedSavingsVersusFresh: Double?
        /// Ratio of current input tokens to fresh-session baseline.
        let transcriptGrowthRatio: Double?
        /// Effective prompt size as a fraction of model context window (0.0–1.0).
        let effectivePromptSizeFraction: Double?
        /// Number of compaction/truncation events already applied to this generation.
        let compactionChurnCount: Int?
    }

    /// Evaluate whether a generation should continue being reused (§6.3).
    ///
    /// Order of evaluation:
    /// 1. Hard guardrails (backstops)
    /// 2. Economic signals (primary authority)
    /// 3. Compaction/truncation churn
    static func evaluate(
        generation: AgentSessionGeneration,
        signals: EconomicSignals? = nil,
        config: BudgetConfig = .default
    ) -> BudgetDecision {
        // ── 1. Threshold checks (Guardrails — backstops, not primary authority) ──

        if generation.turnCount >= config.maxTurns {
            return .compact(reason: "Max turns (\(config.maxTurns)) exceeded")
        }

        if generation.estimatedInputTokens >= config.maxEstimatedInputTokens {
            return .compact(reason: "Max estimated input tokens (\(config.maxEstimatedInputTokens)) exceeded")
        }

        if generation.cumulativePromptTokens >= config.maxCumulativePromptTokens {
            return .invalidate(reason: "Max cumulative prompt tokens (\(config.maxCumulativePromptTokens)) exceeded")
        }

        if generation.cumulativeCostCents >= config.maxCumulativeCostCents {
            return .invalidate(reason: "Max cumulative cost (\(config.maxCumulativeCostCents) cents) exceeded")
        }

        let idleAge = Date().timeIntervalSince(generation.createdAt)
        if idleAge >= config.maxIdleAgeSeconds {
            return .invalidate(reason: "Max idle age (\(Int(config.maxIdleAgeSeconds))s) exceeded")
        }

        // ── 2. Economic signals (Primary decision authority — §6.3) ──

        if let signals = signals {
            // 2a. Effective prompt size approaching model context limit
            if let promptFraction = signals.effectivePromptSizeFraction,
               promptFraction > config.maxEffectivePromptSizeFraction {
                return .compact(reason: "Effective prompt size (\(Int(promptFraction * 100))%) exceeds \(Int(config.maxEffectivePromptSizeFraction * 100))% of context window")
            }

            // 2b. Cache-hit rate is low for a large prompt — reuse isn't paying off
            if let cacheShare = signals.cachedTokenShare,
               cacheShare < config.minCachedTokenShare,
               generation.estimatedInputTokens > 50_000 {
                return .compact(reason: "Low cache hit rate (\(Int(cacheShare * 100))%) with large prompt (\(generation.estimatedInputTokens) tokens)")
            }

            // 2c. Transcript growth exceeds safe ratio
            if let growth = signals.transcriptGrowthRatio,
               growth > config.maxTranscriptGrowthRatio {
                return .compact(reason: "Transcript growth (\(String(format: "%.1f", growth))x) exceeded \(String(format: "%.1f", config.maxTranscriptGrowthRatio))x guardrail")
            }

            // 2d. Normalized savings show reuse is net more expensive than fresh
            if let savings = signals.normalizedSavingsVersusFresh,
               savings < -config.maxReuseCostPenaltyCents {
                return .invalidate(reason: "Reuse cost penalty (\(String(format: "%.1f", -savings))c) exceeded \(String(format: "%.1f", config.maxReuseCostPenaltyCents))c threshold")
            }

            // 2e. Compaction/truncation churn — if already compacted multiple times,
            // further reuse is unlikely to be economical (§6.3)
            if let churn = signals.compactionChurnCount, churn >= 3 {
                return .invalidate(reason: "Compaction churn (\(churn)) suggests reuse is no longer beneficial")
            }
        }

        return .continueReuse
    }
}
