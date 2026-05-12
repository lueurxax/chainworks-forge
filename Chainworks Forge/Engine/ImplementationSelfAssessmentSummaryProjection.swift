import Foundation

enum ImplementationSelfAssessmentSummaryProjection {
    private static let knownStatuses: Set<String> = [
        "invalid",
        "needs_code_fixes",
        "blocked",
        "handoff_required",
        "complete",
        "unknown",
    ]

    static func canonicalSummaryData(from data: Data, artifactName: String?) -> Data? {
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }

        if isCanonicalSummaryObject(object) {
            return canonicalData(from: object)
        }

        if artifactName == "implementation_review_summary",
            let summaryObject = object["implementation_self_assessment_summary"] as? [String: Any],
            isCanonicalSummaryObject(summaryObject)
        {
            return canonicalData(from: summaryObject)
        }

        return nil
    }

    static func scalarFields(fromCanonicalSummaryData data: Data) -> [String: AnyCodableValue]? {
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            isCanonicalSummaryObject(object)
        else {
            return nil
        }

        var fields: [String: AnyCodableValue] = [:]
        for (key, value) in object {
            if let number = value as? NSNumber {
                if CFGetTypeID(number) == CFBooleanGetTypeID() {
                    fields[key] = .bool(number.boolValue)
                } else if floor(number.doubleValue) == number.doubleValue {
                    fields[key] = .int(number.intValue)
                } else {
                    fields[key] = .double(number.doubleValue)
                }
            } else if let boolValue = value as? Bool {
                fields[key] = .bool(boolValue)
            } else if let intValue = value as? Int {
                fields[key] = .int(intValue)
            } else if let doubleValue = value as? Double {
                fields[key] = .double(doubleValue)
            } else if let stringValue = value as? String {
                fields[key] = .string(stringValue)
            }
        }

        return fields
    }

    static func isCanonicalSummaryObject(_ object: [String: Any]) -> Bool {
        guard let status = object["status"] as? String,
            knownStatuses.contains(status)
        else {
            return false
        }

        return object["owner_class_counts"] != nil
            || object["target_stage_summaries"] != nil
            || object["validation_errors"] != nil
            || object["warnings"] != nil
    }

    private static func canonicalData(from object: [String: Any]) -> Data? {
        try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
    }

}

struct ImplementationSelfAssessmentDisplaySummary: Equatable {
    struct Task: Equatable {
        let summary: String
        let owner: String
        let blocking: Bool
        let evidence: String
        let sourcePointer: String?
    }

    struct HandoffTask: Equatable {
        let summary: String
        let owner: String
        let targetStage: String
        let blockingReview: Bool
        let evidence: String
    }

    struct Diagnostic: Equatable {
        let code: String
        let message: String
        let pointer: String
    }

    struct TargetStageSummary: Equatable {
        let targetStage: String
        let count: Int
        let blockingReviewCount: Int
    }

    let status: String
    let implementationComplete: Bool?
    let verificationGreen: Bool?
    let verificationLabel: String
    let remainingCodeTasks: [Task]
    let handoffTasks: [HandoffTask]
    let knownRisks: [String]
    let testsRun: [String]
    let docsImpacted: [String]
    let validationErrors: [Diagnostic]
    let warnings: [Diagnostic]
    let ownerClassCounts: [String: Int]
    let targetStageSummaries: [TargetStageSummary]
    let sourceArtifactName: String
    let evidenceText: String
}

enum ImplementationSelfAssessmentDisplayAdapter {
    static func summary(from run: Run, artifacts: [Artifact]) -> ImplementationSelfAssessmentDisplaySummary? {
        if let data = run.implementationSelfAssessmentSummaryJSON,
           let summary = summary(fromCanonicalData: data, sourceArtifactName: "run.implementation_self_assessment_summary") {
            return summary
        }
        return summary(from: artifacts)
    }

    static func summary(from artifacts: [Artifact]) -> ImplementationSelfAssessmentDisplaySummary? {
        for artifact in artifacts where artifact.name == "implementation_review_summary" {
            guard let data = try? Data(contentsOf: URL(fileURLWithPath: artifact.filePath)),
                  let canonicalData = ImplementationSelfAssessmentSummaryProjection.canonicalSummaryData(
                    from: data,
                    artifactName: artifact.name
                  ),
                  let summary = summary(
                    fromCanonicalData: canonicalData,
                    sourceArtifactName: "implementation_review_summary.implementation_self_assessment_summary"
                  )
            else { continue }
            return summary
        }

        for artifact in artifacts where artifact.name == "implementation_self_assessment" {
            guard let data = try? Data(contentsOf: URL(fileURLWithPath: artifact.filePath)),
                  let canonicalData = ImplementationSelfAssessmentSummaryProjection.canonicalSummaryData(
                    from: data,
                    artifactName: artifact.name
                  ),
                  let summary = summary(fromCanonicalData: canonicalData, sourceArtifactName: artifact.name)
            else { continue }
            return summary
        }

        return nil
    }

    private static func summary(
        fromCanonicalData data: Data,
        sourceArtifactName: String
    ) -> ImplementationSelfAssessmentDisplaySummary? {
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let status = object["status"] as? String
        else { return nil }

        let verificationGreen = object["verification_green"] as? Bool
        let validationErrors = diagnostics(from: object["validation_errors"])
        let warnings = diagnostics(from: object["warnings"])
        let testsRun = strings(from: object["tests_run"])
        let knownRisks = strings(from: object["known_risks"])
        let docsImpacted = strings(from: object["docs_impacted"])
        let remainingCodeTasks = codeTasks(from: object["remaining_code_tasks"])
        let handoffTasks = handoffTasks(from: object["handoff_tasks"])
        let targetStageSummaries = targetSummaries(from: object["target_stage_summaries"])

        var evidenceLines = testsRun + knownRisks
        evidenceLines.append(contentsOf: validationErrors.map { "\($0.pointer): \($0.message)" })
        evidenceLines.append(contentsOf: warnings.map { "\($0.pointer): \($0.message)" })

        return ImplementationSelfAssessmentDisplaySummary(
            status: status,
            implementationComplete: object["implementation_complete"] as? Bool,
            verificationGreen: verificationGreen,
            verificationLabel: verificationLabel(status: status, verificationGreen: verificationGreen),
            remainingCodeTasks: remainingCodeTasks,
            handoffTasks: handoffTasks,
            knownRisks: knownRisks,
            testsRun: testsRun,
            docsImpacted: docsImpacted,
            validationErrors: validationErrors,
            warnings: warnings,
            ownerClassCounts: intDictionary(from: object["owner_class_counts"]),
            targetStageSummaries: targetStageSummaries,
            sourceArtifactName: sourceArtifactName,
            evidenceText: evidenceLines.joined(separator: "\n")
        )
    }

    private static func verificationLabel(status: String, verificationGreen: Bool?) -> String {
        if status == "blocked" {
            return "Blocked"
        }
        guard let verificationGreen else {
            return "Unknown"
        }
        return verificationGreen ? "Green" : "Failed"
    }

    private static func codeTasks(from value: Any?) -> [ImplementationSelfAssessmentDisplaySummary.Task] {
        dictionaries(from: value).map { row in
            ImplementationSelfAssessmentDisplaySummary.Task(
                summary: row["summary"] as? String ?? "",
                owner: row["owner"] as? String ?? "",
                blocking: row["blocking"] as? Bool ?? false,
                evidence: row["evidence"] as? String ?? "",
                sourcePointer: row["source_pointer"] as? String
            )
        }
    }

    private static func handoffTasks(from value: Any?) -> [ImplementationSelfAssessmentDisplaySummary.HandoffTask] {
        dictionaries(from: value).map { row in
            let ownerClass = row["owner_class"] as? String ?? "unknown"
            return ImplementationSelfAssessmentDisplaySummary.HandoffTask(
                summary: row["summary"] as? String ?? "",
                owner: ownerLabel(for: ownerClass),
                targetStage: row["target_stage"] as? String ?? "",
                blockingReview: row["blocking_review"] as? Bool ?? false,
                evidence: row["evidence"] as? String ?? ""
            )
        }
    }

    private static func targetSummaries(from value: Any?) -> [ImplementationSelfAssessmentDisplaySummary.TargetStageSummary] {
        dictionaries(from: value).map { row in
            ImplementationSelfAssessmentDisplaySummary.TargetStageSummary(
                targetStage: row["target_stage"] as? String ?? "",
                count: int(from: row["count"]),
                blockingReviewCount: int(from: row["blocking_review_count"])
            )
        }
    }

    private static func diagnostics(from value: Any?) -> [ImplementationSelfAssessmentDisplaySummary.Diagnostic] {
        dictionaries(from: value).map { row in
            ImplementationSelfAssessmentDisplaySummary.Diagnostic(
                code: row["code"] as? String ?? "",
                message: row["message"] as? String ?? "",
                pointer: row["pointer"] as? String ?? ""
            )
        }
    }

    private static func ownerLabel(for ownerClass: String) -> String {
        switch ownerClass {
        case "code_writer":
            return "Code Writer"
        case "manual_evidence":
            return "Manual Evidence"
        case "release":
            return "Release"
        default:
            return "Human Triage"
        }
    }

    private static func strings(from value: Any?) -> [String] {
        value as? [String] ?? []
    }

    private static func dictionaries(from value: Any?) -> [[String: Any]] {
        value as? [[String: Any]] ?? []
    }

    private static func intDictionary(from value: Any?) -> [String: Int] {
        guard let object = value as? [String: Any] else { return [:] }
        return object.reduce(into: [:]) { partial, element in
            partial[element.key] = int(from: element.value)
        }
    }

    private static func int(from value: Any?) -> Int {
        if let value = value as? Int {
            return value
        }
        if let value = value as? NSNumber {
            return value.intValue
        }
        return 0
    }
}
