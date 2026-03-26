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

        // 4. Copy ALL artifacts organized by category
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

        // 5. Named deliverables directory (§12.2 explicit promised items)
        let deliverablesDir = packDir.appendingPathComponent("deliverables", isDirectory: true)
        try fm.createDirectory(at: deliverablesDir, withIntermediateDirectories: true)

        let namedDeliverables: [(artifactName: String, deliverableName: String)] = [
            ("proposal_current", "proposal-draft.json"),
            ("approved_proposal", "approved-proposal.json"),
            ("implementation_review_summary", "implementation-review-summary.json"),
            ("docs_report", "docs-report.json"),
            ("docs_delta", "docs-delta.json"),
            ("security_report", "security-report.json"),
            ("audit_report", "audit-report.json"),
            ("prepush_review_report", "prepush-review-report.json"),
            ("changed_files_manifest", "diff-summary.json"),
            ("release_manifest", "release-manifest.json"),
            ("git_push_receipt", "git-push-receipt.json"),
            ("release_bundle_manifest", "release-bundle-manifest.json"),
            ("connect_upload_receipt", "connect-upload-receipt.json"),
            ("delivery_receipt", "delivery-receipt.json"),
            ("run_report", "run-report.json"),
            ("tests_result", "tests-result.json"),
            ("orchestrator_summary", "orchestrator-summary.json")
        ]

        for (artifactName, deliverableName) in namedDeliverables {
            if let artifact = allArtifacts.last(where: { $0.name == artifactName }),
               fm.fileExists(atPath: artifact.filePath) {
                let destPath = deliverablesDir.appendingPathComponent(deliverableName)
                try? fm.copyItem(atPath: artifact.filePath, toPath: destPath.path)
                itemCount += 1
            }
        }

        // 6. Stage execution summary
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

        // 7. Agent execution detail
        let agentDetail = run.stageExecutions
            .sorted { $0.startedAt < $1.startedAt }
            .flatMap { stage in
                stage.agentExecutions.map { agent -> [String: Any] in
                    [
                        "stageID": stage.stageID,
                        "agentID": agent.agentID,
                        "agentTitle": agent.agentTitle,
                        "taskName": agent.taskName,
                        "status": agent.status.rawValue,
                        "provider": agent.provider ?? "unknown",
                        "costCents": agent.costCents ?? 0,
                        "repoRevisionBefore": agent.repoRevisionBefore ?? "none",
                        "repoRevisionAfter": agent.repoRevisionAfter ?? "none"
                    ] as [String: Any]
                }
            }

        if let agentData = try? JSONSerialization.data(withJSONObject: agentDetail, options: [.prettyPrinted, .sortedKeys]) {
            try agentData.write(to: packDir.appendingPathComponent("agent-execution-detail.json"))
            itemCount += 1
        }

        // 8. Screenshot checklist template
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
