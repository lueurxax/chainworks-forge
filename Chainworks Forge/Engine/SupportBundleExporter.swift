import Foundation
import SwiftData

@MainActor
struct SupportBundleExporter {
    let modelContext: ModelContext
    let appConfigurationStore: AppConfigurationStore
    let providerRegistry: ProviderRegistry

    func exportBundle() async throws -> URL {
        let exportRoot = appConfigurationStore.configuration.supportBundleExportPath.map {
            URL(fileURLWithPath: $0, isDirectory: true)
        } ?? AppConfiguration.defaultSupportRoot().appendingPathComponent("exports", isDirectory: true)

        try FileManager.default.createDirectory(at: exportRoot, withIntermediateDirectories: true)

        let bundleName = "chainworks-support-\(timestamp())"
        let stagingDirectory = exportRoot.appendingPathComponent(bundleName, isDirectory: true)
        try FileManager.default.createDirectory(at: stagingDirectory, withIntermediateDirectories: true)

        let runs = fetchRuns()
        let latestRun = runs.first

        try writeJSON(appMetadataSummary(), to: stagingDirectory.appendingPathComponent("app-version.json"))
        try writeJSON(configurationSummary(), to: stagingDirectory.appendingPathComponent("configuration-summary.json"))
        try writeJSON(providerHealthSummary(), to: stagingDirectory.appendingPathComponent("provider-health.json"))
        try writeJSON(adapterVersionSummary(), to: stagingDirectory.appendingPathComponent("adapter-versions.json"))
        try writeJSON(runSummary(runs: runs), to: stagingDirectory.appendingPathComponent("run-summary.json"))
        try writeJSON(agentExecutionSummary(run: latestRun), to: stagingDirectory.appendingPathComponent("agent-executions.json"))
        try writeJSON(artifactIndex(run: latestRun), to: stagingDirectory.appendingPathComponent("artifact-index.json"))
        try exportSelectedArtifacts(from: latestRun, to: stagingDirectory.appendingPathComponent("artifacts", isDirectory: true))

        let zipURL = exportRoot.appendingPathComponent("\(bundleName).zip")
        try zipDirectory(stagingDirectory, destination: zipURL)
        try? FileManager.default.removeItem(at: stagingDirectory)
        return zipURL
    }

    private func appMetadataSummary() -> [String: Any] {
        [
            "appVersion": Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "dev",
            "bundleVersion": Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "dev",
            "exportedAt": ISO8601DateFormatter().string(from: Date())
        ]
    }

    private func configurationSummary() -> [String: Any] {
        [
            "appConfiguration": [
                "runStorageBasePath": appConfigurationStore.configuration.runStorageBasePath,
                "worktreeBasePath": appConfigurationStore.configuration.worktreeBasePath ?? "",
                "workflowSourcePath": appConfigurationStore.configuration.workflowSourcePath,
                "agentCatalogSourcePath": appConfigurationStore.configuration.agentCatalogSourcePath,
                "supportBundleExportPath": appConfigurationStore.configuration.supportBundleExportPath ?? "",
                "activeConfigurationSource": appConfigurationStore.configuration.activeConfigurationSource.rawValue
            ],
            "exportedAt": ISO8601DateFormatter().string(from: Date())
        ]
    }

    private func providerHealthSummary() -> [String: Any] {
        [
            "providers": providerRegistry.configuredProviders.map { provider in
                let snapshot = providerRegistry.healthSnapshot(for: provider.id)
                return [
                    "id": provider.id.uuidString,
                    "family": provider.family.rawValue,
                    "displayName": provider.displayName,
                    "transport": provider.transport.rawValue,
                    "defaultModel": provider.defaultModel ?? "",
                    "status": snapshot?.status.rawValue ?? ProviderStatus.unknown.rawValue,
                    "summary": snapshot?.summary ?? "",
                    "blockingIssues": snapshot?.blockingIssues ?? []
                ]
            }
        ]
    }

    private func adapterVersionSummary() -> [String: Any] {
        [
            "providers": providerRegistry.configuredProviders.map { provider in
                [
                    "providerID": provider.id.uuidString,
                    "family": provider.family.rawValue,
                    "displayName": provider.displayName,
                    "adapterVersion": provider.adapterVersion
                ]
            }
        ]
    }

    private func fetchRuns() -> [Run] {
        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        return (try? modelContext.fetch(descriptor)) ?? []
    }

    private func runSummary(runs: [Run]) -> [String: Any] {
        let latestRun = runs.first
        let latestRunSummary: [String: Any]

        if let run = latestRun {
            latestRunSummary = [
                "id": run.id.uuidString,
                "workflowTitle": run.workflowTitle,
                "status": run.status.rawValue,
                "startedAt": ISO8601DateFormatter().string(from: run.startedAt),
                "completedAt": jsonValue(run.completedAt.map { ISO8601DateFormatter().string(from: $0) }),
                "totalCostCents": jsonValue(run.totalCostCents),
                "runtimeTrustLevel": run.runtimeTrustLevel ?? ""
            ]
        } else {
            latestRunSummary = [:]
        }

        return [
            "latestRun": latestRunSummary,
            "blockedRuns": runs.filter { $0.status == .blocked }.count,
            "waitingApprovalRuns": runs.filter { $0.status == .waitingApproval }.count
        ]
    }

    private func agentExecutionSummary(run: Run?) -> [String: Any] {
        guard let run else {
            return ["latestRunAgents": []]
        }

        let agents = run.stageExecutions
            .sorted { $0.startedAt < $1.startedAt }
            .flatMap(\.agentExecutions)
            .sorted { $0.startedAt < $1.startedAt }

        return [
            "runID": run.id.uuidString,
            "latestRunAgents": agents.map { agent in
                [
                    "agentID": agent.agentID,
                    "agentTitle": agent.agentTitle,
                    "taskName": agent.taskName,
                    "status": agent.status.rawValue,
                    "provider": agent.provider,
                    "resolvedModel": agent.resolvedModel ?? "",
                    "effort": agent.effort,
                    "costCents": jsonValue(agent.costCents),
                    "configuredProviderID": jsonValue(agent.configuredProviderID?.uuidString),
                    "adapterVersion": agent.adapterVersion ?? "",
                    "runtimeSessionID": agent.runtimeSessionID ?? "",
                    "providerSessionID": agent.providerSessionID ?? "",
                    "logSnippet": agent.logSnippet ?? ""
                ]
            }
        ]
    }

    private func artifactIndex(run: Run?) -> [String: Any] {
        guard let run else {
            return ["artifacts": []]
        }

        let artifacts = run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)
            .sorted { $0.createdAt > $1.createdAt }

        return [
            "runID": run.id.uuidString,
            "artifacts": artifacts.map { artifact in
                [
                    "id": artifact.id.uuidString,
                    "name": artifact.name,
                    "contractID": artifact.contractID,
                    "format": artifact.format.rawValue,
                    "filePath": artifact.filePath,
                    "sizeBytes": jsonValue(artifact.sizeBytes),
                    "checksumSHA256": artifact.checksumSHA256 ?? "",
                    "stageID": artifact.stageID,
                    "agentID": artifact.agentID,
                    "provider": artifact.provider,
                    "model": artifact.model ?? "",
                    "effort": artifact.effort ?? "",
                    "isPinned": artifact.isPinned,
                    "reportKind": artifact.reportKind ?? ""
                ]
            }
        ]
    }

    private func exportSelectedArtifacts(from run: Run?, to artifactsDirectory: URL) throws {
        guard let run else { return }

        let artifacts = run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)
            .filter { $0.isPinned || $0.reportKind != nil || $0.contractID == "run_report" || $0.contractID == "run_summary" }

        guard !artifacts.isEmpty else { return }
        try FileManager.default.createDirectory(at: artifactsDirectory, withIntermediateDirectories: true)

        for artifact in artifacts {
            let sourceURL = URL(fileURLWithPath: artifact.filePath)
            guard FileManager.default.fileExists(atPath: sourceURL.path) else { continue }
            let destinationURL = artifactsDirectory
                .appendingPathComponent(sanitizedFileName(artifact.name))
            if FileManager.default.fileExists(atPath: destinationURL.path) {
                try? FileManager.default.removeItem(at: destinationURL)
            }
            try FileManager.default.copyItem(at: sourceURL, to: destinationURL)
        }
    }

    private func writeJSON(_ object: Any, to url: URL) throws {
        let data = try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: url, options: .atomic)
    }

    private func zipDirectory(_ source: URL, destination: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.arguments = ["-r", destination.path, source.lastPathComponent]
        process.currentDirectoryURL = source.deletingLastPathComponent()
        try process.run()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw CocoaError(.fileWriteUnknown)
        }
    }

    private func timestamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        return formatter.string(from: Date())
    }

    private func sanitizedFileName(_ name: String) -> String {
        name.replacingOccurrences(of: "/", with: "_")
    }

    private func jsonValue(_ value: Any?) -> Any {
        value ?? NSNull()
    }
}
