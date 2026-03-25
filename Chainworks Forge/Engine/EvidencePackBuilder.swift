import Foundation

// MARK: - EvidencePackBuilder (Proposal 007 — §12.2)

/// Exports a complete dogfood evidence pack for a repo-backed run.
/// Contains: run report, pinned artifacts, proposal, review summary,
/// docs report/delta, diff summary, git push receipt, connect upload receipt,
/// support bundle, and screenshot checklist.
@MainActor
final class EvidencePackBuilder {

    struct EvidencePack: Sendable {
        let exportPath: URL
        let itemCount: Int
        let timestamp: Date
    }

    /// Build and export an evidence pack for a completed run.
    static func export(
        run: Run,
        workspace: RunWorkspace,
        exportDirectory: URL
    ) throws -> EvidencePack {
        let fm = FileManager.default
        let packDir = exportDirectory
            .appendingPathComponent("evidence-pack-\(run.id.uuidString.prefix(8))", isDirectory: true)
        try fm.createDirectory(at: packDir, withIntermediateDirectories: true)

        var itemCount = 0

        // 1. Run metadata
        let metadata: [String: Any] = [
            "runID": run.id.uuidString,
            "workflowID": run.workflowID,
            "workflowTitle": run.workflowTitle,
            "status": run.status.rawValue,
            "startedAt": ISO8601DateFormatter().string(from: run.startedAt),
            "completedAt": run.completedAt.map { ISO8601DateFormatter().string(from: $0) } ?? "in_progress",
            "worktreeRoot": run.worktreeRoot ?? "none",
            "repoIdentifier": run.repoIdentifier ?? "none",
            "baseBranch": run.baseBranch ?? "none",
            "baseRevision": run.baseRevision ?? "none",
            "targetBranch": run.targetBranch ?? "none",
            "releaseMode": run.releaseMode ?? "none"
        ]

        if let jsonData = try? JSONSerialization.data(withJSONObject: metadata, options: [.prettyPrinted, .sortedKeys]) {
            try jsonData.write(to: packDir.appendingPathComponent("run-metadata.json"))
            itemCount += 1
        }

        // 2. Delivery configuration
        if let configData = run.deliveryConfigurationJSON {
            try configData.write(to: packDir.appendingPathComponent("delivery-configuration.json"))
            itemCount += 1
        }

        // 3. Delivery preflight
        if let preflightData = run.deliveryPreflightJSON {
            try preflightData.write(to: packDir.appendingPathComponent("delivery-preflight.json"))
            itemCount += 1
        }

        // 4. Copy all artifacts
        let artifactsDir = packDir.appendingPathComponent("artifacts", isDirectory: true)
        try fm.createDirectory(at: artifactsDir, withIntermediateDirectories: true)

        let allArtifacts = run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)

        for artifact in allArtifacts {
            let sourcePath = artifact.filePath
            if fm.fileExists(atPath: sourcePath) {
                let destName = "\(artifact.stageID)_\(artifact.agentID)_\(artifact.name)"
                    .replacingOccurrences(of: "/", with: "_")
                let destPath = artifactsDir.appendingPathComponent(destName)
                try? fm.copyItem(atPath: sourcePath, toPath: destPath.path)
                itemCount += 1
            }
        }

        // 5. Stage execution summary
        let stageSummary = run.stageExecutions
            .sorted { $0.startedAt < $1.startedAt }
            .map { stage -> [String: Any] in
                [
                    "stageID": stage.stageID,
                    "label": stage.label,
                    "status": stage.status.rawValue,
                    "iteration": stage.iteration,
                    "attemptNumber": stage.attemptNumber,
                    "agentCount": stage.agentExecutions.count,
                    "artifactCount": stage.agentExecutions.flatMap(\.artifacts).count
                ] as [String: Any]
            }

        if let stageData = try? JSONSerialization.data(withJSONObject: stageSummary, options: [.prettyPrinted, .sortedKeys]) {
            try stageData.write(to: packDir.appendingPathComponent("stage-summary.json"))
            itemCount += 1
        }

        // 6. Screenshot checklist template
        let checklist = """
        # Evidence Pack Screenshot Checklist

        ## Happy Path
        - [ ] Start Run preset
        - [ ] Proposal approved
        - [ ] Implementation review green
        - [ ] Manual release gate
        - [ ] Completed run with receipts

        ## Non-Happy Path
        - [ ] Blocked implementation review or blocked release
        - [ ] Recovery sheet / release gate re-entry
        - [ ] Final recovered or cancelled state

        Pack exported: \(ISO8601DateFormatter().string(from: Date()))
        Run: \(run.id.uuidString.prefix(8))
        """
        try checklist.write(to: packDir.appendingPathComponent("screenshot-checklist.md"), atomically: true, encoding: .utf8)
        itemCount += 1

        return EvidencePack(
            exportPath: packDir,
            itemCount: itemCount,
            timestamp: Date()
        )
    }
}
