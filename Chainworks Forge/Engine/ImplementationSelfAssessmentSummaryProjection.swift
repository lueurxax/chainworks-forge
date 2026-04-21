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
