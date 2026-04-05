import Foundation

struct ImplementationFailureArtifactSynthesizer {
    static let progressArtifactName = "implementation_progress"
    static let selfAssessmentArtifactName = "implementation_self_assessment"
    static let changedFilesArtifactName = "changed_files_manifest"
    static let testsResultArtifactName = "tests_result"

    private static let supportedArtifactNames: Set<String> = [
        progressArtifactName,
        selfAssessmentArtifactName,
        changedFilesArtifactName,
        testsResultArtifactName,
    ]

    static func supplementMissingOutputs(
        existingOutputs: [String: Data],
        expectedOutputs: [String],
        agent: ResolvedAgent,
        context: ExecutionContext,
        failureSummary: String
    ) -> [String: Data] {
        guard shouldSynthesize(expectedOutputs: expectedOutputs, agent: agent, context: context) else {
            return existingOutputs
        }

        let changedFiles = collectChangedFiles(in: context.workspace.worktreeRoot)
        let docsImpacted = changedFiles.filter { $0.hasSuffix(".md") || $0.hasPrefix("docs/") }
        let missingOutputs = expectedOutputs.filter { existingOutputs[$0] == nil }
        let progressStatus = changedFiles.isEmpty ? "blocked" : "partial"
        let completedItems: [String] = changedFiles.isEmpty
            ? []
            : ["Execution left partial worktree edits in \(changedFiles.count) file(s)."]

        var outputs = existingOutputs

        if outputs[progressArtifactName] == nil, expectedOutputs.contains(progressArtifactName) {
            outputs[progressArtifactName] = makeJSONData([
                "status": progressStatus,
                "current_phase": "implementation",
                "completed_items": completedItems,
                "deferred_items": missingOutputs.filter { $0 != progressArtifactName },
                "notes": failureSummary,
            ])
        }

        if outputs[selfAssessmentArtifactName] == nil, expectedOutputs.contains(selfAssessmentArtifactName) {
            outputs[selfAssessmentArtifactName] = makeJSONData([
                "seemingly_complete": false,
                "remaining_tasks": remainingTasks(changedFiles: changedFiles, missingOutputs: missingOutputs),
                "known_risks": [failureSummary],
                "tests_run": false,
                "docs_impacted": docsImpacted,
            ])
        }

        if outputs[changedFilesArtifactName] == nil, expectedOutputs.contains(changedFilesArtifactName) {
            outputs[changedFilesArtifactName] = makeJSONData([
                "files": changedFiles,
                "summary": changedFiles.isEmpty
                    ? "Execution stopped before producing a changed-files report."
                    : "Execution stopped after leaving partial edits in the worktree.",
            ])
        }

        if outputs[testsResultArtifactName] == nil, expectedOutputs.contains(testsResultArtifactName) {
            outputs[testsResultArtifactName] = makeJSONData([
                "green": false,
                "summary": "Execution stopped before the required test report was written: \(failureSummary)",
            ])
        }

        return outputs
    }

    static func containsRecoverableArtifactSet(_ outputs: [String: Data]) -> Bool {
        supportedArtifactNames.allSatisfy { outputs[$0] != nil }
    }

    private static func shouldSynthesize(
        expectedOutputs: [String],
        agent: ResolvedAgent,
        context: ExecutionContext
    ) -> Bool {
        guard agent.worktreeWriteEnabled, context.workspace.worktreeRoot != nil else {
            return false
        }
        return !supportedArtifactNames.isDisjoint(with: expectedOutputs)
    }

    private static func remainingTasks(changedFiles: [String], missingOutputs: [String]) -> [String] {
        var tasks = missingOutputs.map { "Produce required artifact: \($0)" }
        if changedFiles.isEmpty {
            tasks.append("Resume implementation from a clean blocked state.")
        } else {
            tasks.append("Review partial worktree edits and continue implementation.")
        }
        tasks.append("Run verification and emit a canonical tests_result artifact.")
        return tasks
    }

    private static func collectChangedFiles(in worktreeRoot: URL?) -> [String] {
        guard let worktreeRoot else { return [] }
        guard FileManager.default.fileExists(atPath: worktreeRoot.path) else { return [] }

        do {
            let output = try runGitStatus(in: worktreeRoot)
            return parseStatusOutput(output)
        } catch {
            return []
        }
    }

    private static func runGitStatus(in directory: URL) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["status", "--porcelain", "--untracked-files=all"]
        process.currentDirectoryURL = directory

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            let error = String(data: stderr.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
            throw NSError(
                domain: "ImplementationFailureArtifactSynthesizer",
                code: Int(process.terminationStatus),
                userInfo: [NSLocalizedDescriptionKey: error]
            )
        }

        return String(data: stdout.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
    }

    private static func parseStatusOutput(_ output: String) -> [String] {
        output
            .split(separator: "\n")
            .map { line -> String in
                let raw = String(line).trimmingCharacters(in: .whitespaces)
                if let range = raw.range(of: " -> ") {
                    return String(raw[range.upperBound...]).trimmingCharacters(in: .whitespaces)
                }
                if raw.count > 3 {
                    return String(raw.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                }
                return raw
            }
            .filter { !$0.isEmpty }
    }

    private static func makeJSONData(_ object: [String: Any]) -> Data {
        (try? JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys])) ?? Data("{}".utf8)
    }
}
