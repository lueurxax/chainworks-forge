import Foundation
import SwiftData

/// Orchestrates the V1 Steward meta-workflow (steps 1-8).
/// Runs entirely over persisted run data. No in-app Steward UI.
@MainActor
final class StewardAnalysisService {
    let modelContext: ModelContext
    let stewardConfig: StewardConfig
    let executor: AgentExecutor
    let catalog: AgentCatalog?

    init(
        modelContext: ModelContext,
        stewardConfig: StewardConfig,
        executor: AgentExecutor,
        catalog: AgentCatalog? = nil
    ) {
        self.modelContext = modelContext
        self.stewardConfig = stewardConfig
        self.executor = executor
        self.catalog = catalog
    }

    /// Run a complete V1 Steward analysis (meta-workflow steps 1-8).
    func runAnalysis() async throws -> StewardAnalysis {
        let analysisID = UUID()
        let windows = stewardConfig.windows

        // Step 1: Query completed runs
        let fetchDescriptor = FetchDescriptor<Run>(
            sortBy: [SortDescriptor(\.startedAt, order: .reverse)]
        )
        let allRuns = try modelContext.fetch(fetchDescriptor)
        let completedRuns = allRuns.filter { $0.status == .completed }

        // Step 2: Split into observation and baseline windows
        let (observationRuns, baselineRuns) = CohortClassifier.splitWindows(
            runs: completedRuns,
            observationSize: windows.observationWindowSize,
            baselineSize: windows.baselineWindowSize,
            maximumAgeDays: windows.maximumWindowAgeDays
        )

        // Step 3: Classify cohort quality
        let cohortQuality = CohortClassifier.classifyQuality(runs: observationRuns)
        let cohortKeys = extractCohortKeys(from: observationRuns)

        // Step 4: Collect metrics
        let metricsCollector = MetricsCollector(modelContext: modelContext)
        let observationMetrics = metricsCollector.collectMetrics(for: observationRuns)
        let baselineMetrics = metricsCollector.collectMetrics(for: baselineRuns)

        // Step 5: Detect anomalies
        let anomalyDetector = AnomalyDetector()
        let signals = anomalyDetector.detect(
            observation: observationMetrics,
            baseline: baselineMetrics,
            thresholds: stewardConfig.thresholds,
            minimumWindowSize: windows.minimumWindowSize,
            analysisID: analysisID,
            observationRunIDs: observationRuns.map(\.id),
            cohortQuality: cohortQuality
        )

        // Step 6: Build dossiers for implicated runs
        let dossierBuilder = RunDossierBuilder(modelContext: modelContext)
        let implicatedRunIDs = Set(signals.flatMap(\.implicatedRunIDs))
        let implicatedRuns = observationRuns.filter { implicatedRunIDs.contains($0.id) }
        let dossiers = dossierBuilder.buildDossiers(
            for: implicatedRuns.isEmpty ? Array(observationRuns.prefix(5)) : implicatedRuns
        )

        // Step 7: Write deterministic artifacts to disk
        let workspacePath = stewardWorkspacePath(for: analysisID)
        try FileManager.default.createDirectory(atPath: workspacePath, withIntermediateDirectories: true)

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601

        let metricsPath = "\(workspacePath)/metrics-window.json"
        try encoder.encode(observationMetrics).write(to: URL(fileURLWithPath: metricsPath))

        let baselinePath = "\(workspacePath)/baseline-window.json"
        try encoder.encode(baselineMetrics).write(to: URL(fileURLWithPath: baselinePath))

        let dossiersDir = "\(workspacePath)/dossiers"
        try FileManager.default.createDirectory(atPath: dossiersDir, withIntermediateDirectories: true)
        for dossier in dossiers {
            let dossierPath = "\(dossiersDir)/\(dossier.runID.uuidString).json"
            try encoder.encode(dossier).write(to: URL(fileURLWithPath: dossierPath))
        }

        if !signals.isEmpty {
            let alertsPath = "\(workspacePath)/degradation-alerts.json"
            try encoder.encode(signals).write(to: URL(fileURLWithPath: alertsPath))
        }

        // Compute provenance hashes
        let workflowCatalogHash: String
        if let catalog {
            workflowCatalogHash = (try? DefinitionHasher.hash(catalog).sha256) ?? "unknown"
        } else {
            workflowCatalogHash = "no-catalog"
        }
        let stewardConfigHash = (try? DefinitionHasher.hash(stewardConfig).sha256) ?? "unknown"

        let reportPath = "\(workspacePath)/health-report.json"

        let isInconclusive = observationRuns.count < windows.minimumWindowSize
            || baselineRuns.count < windows.minimumWindowSize
        let analysisStatus: AnalysisStatus = isInconclusive ? .inconclusive : .completed

        // Step 8: Persist StewardAnalysis
        let analysis = StewardAnalysis(
            id: analysisID,
            windowStart: observationMetrics.windowStart,
            windowEnd: observationMetrics.windowEnd,
            runCount: observationRuns.count,
            cohortKeys: cohortKeys,
            cohortQuality: cohortQuality,
            metricsSnapshotPath: metricsPath,
            baselineSnapshotPath: baselinePath,
            degradationsDetected: signals.count,
            reportArtifactPath: reportPath,
            status: analysisStatus,
            workflowCatalogSnapshotHash: workflowCatalogHash,
            stewardConfigSnapshotHash: stewardConfigHash
        )
        modelContext.insert(analysis)

        // Create run links
        for run in observationRuns {
            let role: RunRole = implicatedRunIDs.contains(run.id) ? .implicated : .context
            let link = StewardAnalysisRunLink(analysisID: analysisID, runID: run.id, role: role)
            link.analysis = analysis
            modelContext.insert(link)
        }
        for run in baselineRuns {
            let link = StewardAnalysisRunLink(analysisID: analysisID, runID: run.id, role: .baseline)
            link.analysis = analysis
            modelContext.insert(link)
        }

        // Create recommendations from degradation signals
        for signal in signals {
            let prefix = cohortQuality == .weak ? "[WEAK COHORT] " : ""
            let recommendation = StewardRecommendation(
                category: .other,
                summary: "\(prefix)\(signal.metricFamily.uppercased()): \(signal.metricName) degraded by \(Int(signal.deltaPercentage * 100))%",
                targetMetric: signal.metricName,
                confidenceLevel: CohortClassifier.confidenceForQuality(cohortQuality),
                status: .proposed
            )
            recommendation.analysis = analysis
            modelContext.insert(recommendation)
        }

        try modelContext.save()
        return analysis
    }

    // MARK: - Helpers

    private func stewardWorkspacePath(for analysisID: UUID) -> String {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        return appSupport.appendingPathComponent("Chainworks Forge/steward/analyses/\(analysisID.uuidString)").path
    }

    private func extractCohortKeys(from runs: [Run]) -> [String: String] {
        var keys: [String: String] = [:]
        if let first = runs.first {
            if let wf = first.workflowFamily { keys["workflowFamily"] = wf }
            if let rc = first.riskClass { keys["riskClass"] = rc.rawValue }
        }
        return keys
    }
}
