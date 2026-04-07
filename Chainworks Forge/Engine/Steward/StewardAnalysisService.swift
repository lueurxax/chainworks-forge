import Foundation
import SwiftData

/// Orchestrates the V1 Steward meta-workflow (steps 1-8).
/// Runs entirely over persisted run data. No in-app Steward UI.
///
/// Proposal 003 contract obligations fulfilled:
/// - REQ-003: Validates `steward_config.yaml` before running.
/// - REQ-006: Partitions by primary cohort key `(workflowFamily, riskClass)`.
/// - REQ-009: Executes `system_steward` agent and writes `health-report.json`.
/// - REQ-010: Executes `steward_auditor` agent and writes audit report.
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

    // MARK: - Errors

    enum AnalysisError: Error, LocalizedError {
        case configValidationFailed([ValidationIssue])

        var errorDescription: String? {
            switch self {
            case .configValidationFailed(let issues):
                let errors = issues.filter { $0.severity == .error }
                return "Steward config validation failed with \(errors.count) error(s): \(errors.map(\.message).joined(separator: "; "))"
            }
        }
    }

    /// Run a complete V1 Steward analysis (meta-workflow steps 1-8).
    func runAnalysis() async throws -> StewardAnalysis {
        // REQ-003: Validate steward_config before running.
        let validationIssues = YAMLValidator.validateStewardConfig(stewardConfig)
        let errors = validationIssues.filter { $0.severity == .error }
        if !errors.isEmpty {
            throw AnalysisError.configValidationFailed(validationIssues)
        }

        let analysisID = UUID()
        let windows = stewardConfig.windows

        // Step 1: Query completed runs
        let fetchDescriptor = FetchDescriptor<Run>(
            sortBy: [SortDescriptor(\.startedAt, order: .reverse)]
        )
        let allRuns = try modelContext.fetch(fetchDescriptor)
        let completedRuns = allRuns.filter { $0.status == .completed }

        // REQ-006: Partition by primary cohort key `(workflowFamily, riskClass)`.
        // Runs with different primary keys are NEVER compared directly.
        let primaryCohortRuns = selectPrimaryCohort(from: completedRuns)

        // Step 2: Split into observation and baseline windows (over the primary cohort)
        let (observationRuns, baselineRuns) = CohortClassifier.splitWindows(
            runs: primaryCohortRuns,
            observationSize: windows.observationWindowSize,
            baselineSize: windows.baselineWindowSize,
            maximumAgeDays: windows.maximumWindowAgeDays
        )

        // REQ-006: Log `sample_too_small` event when windows are insufficient.
        let isInconclusive = observationRuns.count < windows.minimumWindowSize
            || baselineRuns.count < windows.minimumWindowSize
        if isInconclusive {
            ForgeLogger.steward.info("sample_too_small: observation=\(observationRuns.count), baseline=\(baselineRuns.count), minimum=\(windows.minimumWindowSize). Analysis will be marked inconclusive.")
        }

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

        // Step 7a: Write deterministic artifacts to disk
        let workspacePath = stewardWorkspacePath(for: analysisID)
        try FileManager.default.createDirectory(atPath: workspacePath, withIntermediateDirectories: true)

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601

        let metricsPath = "\(workspacePath)/metrics-window.json"
        let metricsData = try encoder.encode(observationMetrics)
        try metricsData.write(to: URL(fileURLWithPath: metricsPath))

        let baselinePath = "\(workspacePath)/baseline-window.json"
        let baselineData = try encoder.encode(baselineMetrics)
        try baselineData.write(to: URL(fileURLWithPath: baselinePath))

        let dossiersDir = "\(workspacePath)/dossiers"
        try FileManager.default.createDirectory(atPath: dossiersDir, withIntermediateDirectories: true)
        let dossiersData = try encoder.encode(dossiers)
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
        let auditReportPath = "\(workspacePath)/audit-report.json"

        // Step 7b — REQ-009: Execute system_steward agent and write health-report.json.
        var healthReportData: Data?
        if let resolvedSteward = resolveAgent(id: "system_steward") {
            let stewardInputs: [String: Data] = [
                "metrics_window": metricsData,
                "baseline_window": baselineData,
                "implicated_run_dossiers": dossiersData,
            ]

            let task = AgentTask(
                agent: "system_steward",
                task: "sdlc_health_analysis",
                inputs: ["metrics_window", "baseline_window", "implicated_run_dossiers"],
                outputs: ["sdlc_health_report"]
            )

            let stewardWorkspace = RunWorkspace(
                runID: analysisID,
                workspaceRoot: URL(fileURLWithPath: workspacePath),
                artifactRoot: URL(fileURLWithPath: workspacePath).appendingPathComponent("artifacts"),
                worktreeRoot: nil
            )

            let context = ExecutionContext(
                workspace: stewardWorkspace,
                stageID: "steward_analysis",
                ownerExecutionLineageID: analysisID,
                iteration: 1,
                attemptNumber: 1,
                inputArtifacts: stewardInputs,
                variables: [:],
                ideaBody: "",
                providerBinding: nil
            )

            do {
                let result = try await executor.execute(task: task, agent: resolvedSteward, context: context)
                if result.succeeded, let reportData = result.outputs["sdlc_health_report"] {
                    try reportData.write(to: URL(fileURLWithPath: reportPath))
                    healthReportData = reportData
                    ForgeLogger.steward.info("health-report.json written to \(reportPath)")
                }
            } catch {
                ForgeLogger.steward.error("system_steward execution failed: \(error.localizedDescription)")
            }
        } else {
            ForgeLogger.steward.info("system_steward agent not found in catalog — skipping agentic analysis.")
        }

        // Step 7c — REQ-010: Execute steward_auditor and write audit report.
        var auditArtifactPath: String?
        if let resolvedAuditor = resolveAgent(id: "steward_auditor"),
           let healthReport = healthReportData {
            let auditorInputs: [String: Data] = [
                "sdlc_health_report": healthReport,
                "metrics_window": metricsData,
                "baseline_window": baselineData,
                "implicated_run_dossiers": dossiersData,
            ]

            let task = AgentTask(
                agent: "steward_auditor",
                task: "challenge_steward_analysis",
                inputs: ["sdlc_health_report", "metrics_window", "baseline_window", "implicated_run_dossiers"],
                outputs: ["stewardship_audit_report"]
            )

            let auditWorkspace = RunWorkspace(
                runID: analysisID,
                workspaceRoot: URL(fileURLWithPath: workspacePath),
                artifactRoot: URL(fileURLWithPath: workspacePath).appendingPathComponent("artifacts"),
                worktreeRoot: nil
            )

            let context = ExecutionContext(
                workspace: auditWorkspace,
                stageID: "steward_audit",
                ownerExecutionLineageID: analysisID,
                iteration: 1,
                attemptNumber: 1,
                inputArtifacts: auditorInputs,
                variables: [:],
                ideaBody: "",
                providerBinding: nil
            )

            do {
                let result = try await executor.execute(task: task, agent: resolvedAuditor, context: context)
                if result.succeeded, let auditData = result.outputs["stewardship_audit_report"] {
                    try auditData.write(to: URL(fileURLWithPath: auditReportPath))
                    auditArtifactPath = auditReportPath
                    ForgeLogger.steward.info("audit-report.json written to \(auditReportPath)")
                }
            } catch {
                ForgeLogger.steward.error("steward_auditor execution failed: \(error.localizedDescription)")
            }
        } else if healthReportData != nil {
            ForgeLogger.steward.info("steward_auditor agent not found in catalog — skipping audit.")
        }

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
            auditArtifactPath: auditArtifactPath,
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

    // MARK: - Primary Cohort Selection (Proposal 003 — REQ-006)

    /// Select the most populous primary cohort from completed runs.
    /// Primary grouping key: `(workflowFamily, riskClass)`.
    /// Runs with different primary keys are NEVER compared directly.
    private func selectPrimaryCohort(from completedRuns: [Run]) -> [Run] {
        // Group by primary key
        let groups = Dictionary(grouping: completedRuns) { run -> String in
            let wf = run.workflowFamily ?? "default"
            let rc = (run.riskClass ?? .standard).rawValue
            return "\(wf)|\(rc)"
        }

        // Select the largest group for analysis
        guard let largest = groups.max(by: { $0.value.count < $1.value.count }) else {
            return completedRuns
        }

        if groups.count > 1 {
            ForgeLogger.steward.info("Found \(groups.count) primary cohort groups. Analyzing largest: '\(largest.key)' (\(largest.value.count) runs). Other groups excluded per cohorting contract.")
        }

        return largest.value
    }

    // MARK: - Agent Resolution (Proposal 003 — REQ-009, REQ-010)

    /// Resolve an agent from the catalog into a `ResolvedAgent` for execution.
    private func resolveAgent(id agentID: String) -> ResolvedAgent? {
        guard let catalog else { return nil }
        guard let agentDef = catalog.agents.first(where: { $0.id == agentID }) else { return nil }
        guard let backend = catalog.backendProfiles[agentDef.backendProfile] else {
            ForgeLogger.steward.error("Backend profile '\(agentDef.backendProfile)' not found for agent '\(agentID)'")
            return nil
        }
        return ResolvedAgent(
            id: agentDef.id,
            title: agentDef.title,
            mode: agentDef.mode,
            backendProfileID: agentDef.backendProfile,
            provider: backend.provider,
            model: backend.model,
            effort: backend.effort,
            maxTurns: backend.maxTurns,
            temperature: backend.temperature,
            permissionProfile: agentDef.permissionProfile,
            mcpProfileID: agentDef.mcpProfile,
            skillRef: agentDef.skillRef,
            skillRole: agentDef.skillRole,
            prompt: agentDef.prompt,
            outputContract: agentDef.outputContract,
            requiresHumanApproval: agentDef.requiresHumanApproval,
            inputs: agentDef.inputs,
            outputs: agentDef.outputs
        )
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
