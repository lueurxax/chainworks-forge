import Foundation

/// Emitted by the anomaly detector when a metric crosses its threshold.
/// Produced by deterministic code, not by an LLM agent.
/// Validated by unit tests, not by the runtime catalog contract validator.
struct DegradationSignal: Codable, Hashable, Sendable {
    let analysisID: UUID
    let metricName: String
    let metricFamily: String       // "timing", "rework", "quality", "cost", "stability"
    let observedValue: Double
    let baselineValue: Double
    let deltaPercentage: Double
    let thresholdUsed: Double
    let implicatedRunIDs: [UUID]
    let severity: String           // "high", "medium", "low"
    let likelyCauses: [String]
    let confidence: String         // "high", "medium", "low"
}
