import Foundation

/// Deterministic anomaly detector.
/// Compares observation metrics against baseline using configurable thresholds.
/// Does NOT use LLM — all detection is arithmetic.
struct AnomalyDetector: Sendable {

    /// Detect anomalies by comparing observation vs baseline metrics.
    /// Returns empty array if sample size < minimumWindowSize.
    func detect(
        observation: MetricsSnapshot,
        baseline: MetricsSnapshot,
        thresholds: [String: ThresholdEntry],
        minimumWindowSize: Int,
        analysisID: UUID,
        observationRunIDs: [UUID],
        cohortQuality: CohortQuality
    ) -> [DegradationSignal] {
        // Refuse to produce findings with insufficient data (Proposal 003 — REQ-006).
        // The anomaly detector must refuse to flag a degradation when sample size < minimumWindowSize
        // and log a `sample_too_small` event instead of silently returning empty results.
        guard observation.runCount >= minimumWindowSize,
              baseline.runCount >= max(minimumWindowSize, 3) else {
            print("[Steward] sample_too_small: observation=\(observation.runCount), baseline=\(baseline.runCount), minimum=\(minimumWindowSize). Refusing to produce findings.")
            return []
        }

        var signals: [DegradationSignal] = []
        let maxConfidence = CohortClassifier.confidenceForQuality(cohortQuality)

        // Timing: lead time
        if let obsLeadTime = observation.leadTimeMedianSeconds,
           let baseLeadTime = baseline.leadTimeMedianSeconds,
           baseLeadTime > 0,
           let threshold = thresholds["timing"] {
            let delta = (obsLeadTime - baseLeadTime) / baseLeadTime
            if checkThreshold(delta: delta, threshold: threshold) {
                signals.append(DegradationSignal(
                    analysisID: analysisID,
                    metricName: "lead_time_median",
                    metricFamily: "timing",
                    observedValue: obsLeadTime,
                    baselineValue: baseLeadTime,
                    deltaPercentage: delta,
                    thresholdUsed: threshold.trigger,
                    implicatedRunIDs: observationRunIDs,
                    severity: severity(for: delta),
                    likelyCauses: [],
                    confidence: maxConfidence.rawValue
                ))
            }
        }

        // Rework: proposal loop mean
        if baseline.proposalLoopMean > 0, let threshold = thresholds["rework"] {
            let delta = (observation.proposalLoopMean - baseline.proposalLoopMean) / baseline.proposalLoopMean
            if checkThreshold(delta: delta, threshold: threshold) {
                signals.append(DegradationSignal(
                    analysisID: analysisID,
                    metricName: "proposal_loop_mean",
                    metricFamily: "rework",
                    observedValue: observation.proposalLoopMean,
                    baselineValue: baseline.proposalLoopMean,
                    deltaPercentage: delta,
                    thresholdUsed: threshold.trigger,
                    implicatedRunIDs: observationRunIDs,
                    severity: severity(for: delta),
                    likelyCauses: [],
                    confidence: maxConfidence.rawValue
                ))
            }
        }

        // Quality: approval rejection rate
        if baseline.approvalRejectionRate > 0, let threshold = thresholds["quality"] {
            let ratio = observation.approvalRejectionRate / baseline.approvalRejectionRate
            if ratio >= threshold.trigger {
                signals.append(DegradationSignal(
                    analysisID: analysisID,
                    metricName: "approval_rejection_rate",
                    metricFamily: "quality",
                    observedValue: observation.approvalRejectionRate,
                    baselineValue: baseline.approvalRejectionRate,
                    deltaPercentage: ratio - 1.0,
                    thresholdUsed: threshold.trigger,
                    implicatedRunIDs: observationRunIDs,
                    severity: severity(for: ratio - 1.0),
                    likelyCauses: [],
                    confidence: maxConfidence.rawValue
                ))
            }
        }

        // Cost: cost per run median
        if let obsCost = observation.costPerRunMedianCents,
           let baseCost = baseline.costPerRunMedianCents,
           baseCost > 0,
           let threshold = thresholds["cost"] {
            let delta = Double(obsCost - baseCost) / Double(baseCost)
            if checkThreshold(delta: delta, threshold: threshold) {
                signals.append(DegradationSignal(
                    analysisID: analysisID,
                    metricName: "cost_per_run_median",
                    metricFamily: "cost",
                    observedValue: Double(obsCost),
                    baselineValue: Double(baseCost),
                    deltaPercentage: delta,
                    thresholdUsed: threshold.trigger,
                    implicatedRunIDs: observationRunIDs,
                    severity: severity(for: delta),
                    likelyCauses: [],
                    confidence: maxConfidence.rawValue
                ))
            }
        }

        // Stability: failed run rate
        if baseline.failedRunRate > 0, let threshold = thresholds["stability"] {
            let ratio = observation.failedRunRate / baseline.failedRunRate
            if ratio >= threshold.trigger {
                signals.append(DegradationSignal(
                    analysisID: analysisID,
                    metricName: "failed_run_rate",
                    metricFamily: "stability",
                    observedValue: observation.failedRunRate,
                    baselineValue: baseline.failedRunRate,
                    deltaPercentage: ratio - 1.0,
                    thresholdUsed: threshold.trigger,
                    implicatedRunIDs: observationRunIDs,
                    severity: severity(for: ratio - 1.0),
                    likelyCauses: [],
                    confidence: maxConfidence.rawValue
                ))
            }
        }

        return signals
    }

    // MARK: - Private helpers

    private func checkThreshold(delta: Double, threshold: ThresholdEntry) -> Bool {
        switch threshold.method {
        case "median_percentage", "mean_percentage":
            return delta >= threshold.trigger
        case "ratio":
            return (delta + 1.0) >= threshold.trigger
        default:
            return false
        }
    }

    private func severity(for delta: Double) -> String {
        if delta >= 1.0 { return "high" }
        if delta >= 0.5 { return "medium" }
        return "low"
    }
}
