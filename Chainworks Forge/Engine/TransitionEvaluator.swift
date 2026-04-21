import Foundation

// MARK: - TransitionEvaluator (ARCH-031: canonical workflow patterns only)

/// Evaluates transition conditions at runtime.
/// Supports ONLY the canonical patterns defined in ARCH-031:
///   - `always` (when: 'true')
///   - `artifactExists` (when: exists('name'))
///   - `approvalGranted` (when: approval.granted == true)
///   - `approvalRejected` (when: approval.rejected == true)
///   - `expression`: artifact.field {==,!=,<,<=,>,>=} value/vars.X, `and`, `or`
///   - vars.* substituted at RUNTIME from RunPlan.variables
struct TransitionEvaluator {

    /// Context provided to the evaluator at runtime.
    struct EvaluationContext: Sendable {
        /// Names of artifacts that have been produced so far in this run.
        let producedArtifactNames: Set<String>
        /// Whether approval has been granted for the current stage.
        let approvalGranted: Bool
        /// Whether approval has been rejected for the current stage.
        let approvalRejected: Bool
        /// Runtime variables (from RunPlan.variables, may be mutated by loop counters).
        let variables: [String: AnyCodableValue]
        /// Artifact metadata for field-level expression checks.
        /// Key: artifact name, Value: dictionary of field values.
        let artifactFields: [String: [String: AnyCodableValue]]
    }

    /// Evaluate a single transition condition.
    /// Returns `true` if the transition should be taken.
    static func evaluate(
        _ condition: TransitionCondition,
        context: EvaluationContext
    ) -> Bool {
        switch condition {
        case .always:
            return true

        case .artifactExists(let name):
            return context.producedArtifactNames.contains(name)

        case .approvalGranted:
            return context.approvalGranted

        case .approvalRejected:
            return context.approvalRejected

        case .expression(let expr):
            return evaluateExpression(expr, context: context)
        }
    }

    /// Find the first transition whose condition is satisfied.
    static func evaluateFirst(
        transitions: [ExecutableTransition],
        context: EvaluationContext
    ) -> ExecutableTransition? {
        transitions.first { evaluate($0.condition, context: context) }
    }

    // MARK: - Expression Parser (ARCH-031 canonical patterns)

    /// Parse and evaluate a canonical expression string.
    /// Supported syntax:
    ///   - `artifact.field == value`
    ///   - `artifact.field > value`
    ///   - `artifact.field >= value`
    ///   - `artifact.field < value`
    ///   - `artifact.field <= value`
    ///   - `artifact.field != value`
    ///   - `vars.name == value`
    ///   - `expr and expr`
    ///   - `expr or expr`
    ///   - `exists('name')`
    ///   - `approval.granted == true`
    ///   - `approval.rejected == true`
    ///   - `true` / `'true'`
    private static func evaluateExpression(
        _ expr: String,
        context: EvaluationContext
    ) -> Bool {
        let trimmed = expr.trimmingCharacters(in: .whitespaces)

        // Handle 'true' / true literals
        if trimmed == "true" || trimmed == "'true'" {
            return true
        }
        if trimmed == "false" || trimmed == "'false'" {
            return false
        }

        // Handle 'and' / 'or' connectives (split on top-level only)
        if let andResult = splitConnective(trimmed, connective: " and ") {
            return evaluateExpression(andResult.lhs, context: context)
                && evaluateExpression(andResult.rhs, context: context)
        }
        if let orResult = splitConnective(trimmed, connective: " or ") {
            return evaluateExpression(orResult.lhs, context: context)
                || evaluateExpression(orResult.rhs, context: context)
        }

        // Handle exists('name')
        if trimmed.hasPrefix("exists(") && trimmed.hasSuffix(")") {
            let inner = String(trimmed.dropFirst(7).dropLast(1))
                .trimmingCharacters(in: CharacterSet(charactersIn: "'\""))
            return context.producedArtifactNames.contains(inner)
        }

        // Handle approval.granted == true / approval.rejected == true
        if trimmed == "approval.granted == true" {
            return context.approvalGranted
        }
        if trimmed == "approval.rejected == true" {
            return context.approvalRejected
        }

        // Handle comparison expressions: lhs op rhs
        if let comparison = parseComparison(trimmed) {
            let lhsValue = resolveValue(comparison.lhs, context: context)
            let rhsValue = resolveValue(comparison.rhs, context: context)
            return applyOperator(lhsValue, comparison.op, rhsValue)
        }

        // Unrecognized expression: fail closed (return false)
        return false
    }

    // MARK: - Connective Splitting

    private struct ConnectiveSplit {
        let lhs: String
        let rhs: String
    }

    /// Split on a connective keyword, respecting parentheses depth.
    /// Only splits on the FIRST occurrence at depth 0.
    private static func splitConnective(
        _ expr: String,
        connective: String
    ) -> ConnectiveSplit? {
        // Simple case: find connective outside of parentheses
        var depth = 0
        let chars = Array(expr)
        let connChars = Array(connective)
        let connLen = connChars.count

        for i in 0..<chars.count {
            if chars[i] == "(" { depth += 1 }
            else if chars[i] == ")" { depth -= 1 }

            if depth == 0 && i + connLen <= chars.count {
                let slice = Array(chars[i..<min(i + connLen, chars.count)])
                if slice == connChars {
                    let lhs = String(chars[0..<i])
                    let rhs = String(chars[(i + connLen)...])
                    if !lhs.trimmingCharacters(in: .whitespaces).isEmpty
                        && !rhs.trimmingCharacters(in: .whitespaces).isEmpty {
                        return ConnectiveSplit(lhs: lhs, rhs: rhs)
                    }
                }
            }
        }
        return nil
    }

    // MARK: - Comparison Parsing

    private struct Comparison {
        let lhs: String
        let op: ComparisonOp
        let rhs: String
    }

    private enum ComparisonOp {
        case equal       // ==
        case notEqual    // !=
        case lessThan    // <
        case lessOrEqual // <=
        case greaterThan // >
        case greaterOrEqual // >=
    }

    private static func parseComparison(_ expr: String) -> Comparison? {
        // Try <= before < and != before ==
        if let range = expr.range(of: " <= ") {
            let lhs = String(expr[expr.startIndex..<range.lowerBound]).trimmingCharacters(in: .whitespaces)
            let rhs = String(expr[range.upperBound...]).trimmingCharacters(in: .whitespaces)
            return Comparison(lhs: lhs, op: .lessOrEqual, rhs: rhs)
        }
        // Try >= first (before >)
        if let range = expr.range(of: " >= ") {
            let lhs = String(expr[expr.startIndex..<range.lowerBound]).trimmingCharacters(in: .whitespaces)
            let rhs = String(expr[range.upperBound...]).trimmingCharacters(in: .whitespaces)
            return Comparison(lhs: lhs, op: .greaterOrEqual, rhs: rhs)
        }
        if let range = expr.range(of: " != ") {
            let lhs = String(expr[expr.startIndex..<range.lowerBound]).trimmingCharacters(in: .whitespaces)
            let rhs = String(expr[range.upperBound...]).trimmingCharacters(in: .whitespaces)
            return Comparison(lhs: lhs, op: .notEqual, rhs: rhs)
        }
        // Try ==
        if let range = expr.range(of: " == ") {
            let lhs = String(expr[expr.startIndex..<range.lowerBound]).trimmingCharacters(in: .whitespaces)
            let rhs = String(expr[range.upperBound...]).trimmingCharacters(in: .whitespaces)
            return Comparison(lhs: lhs, op: .equal, rhs: rhs)
        }
        if let range = expr.range(of: " < ") {
            let lhs = String(expr[expr.startIndex..<range.lowerBound]).trimmingCharacters(in: .whitespaces)
            let rhs = String(expr[range.upperBound...]).trimmingCharacters(in: .whitespaces)
            return Comparison(lhs: lhs, op: .lessThan, rhs: rhs)
        }
        // Try >
        if let range = expr.range(of: " > ") {
            let lhs = String(expr[expr.startIndex..<range.lowerBound]).trimmingCharacters(in: .whitespaces)
            let rhs = String(expr[range.upperBound...]).trimmingCharacters(in: .whitespaces)
            return Comparison(lhs: lhs, op: .greaterThan, rhs: rhs)
        }
        return nil
    }

    // MARK: - Value Resolution

    /// Resolve a value reference to an AnyCodableValue.
    /// Supports: vars.name, artifact.field, literal int/double/string/bool
    private static func resolveValue(
        _ ref: String,
        context: EvaluationContext
    ) -> AnyCodableValue {
        let trimmed = ref.trimmingCharacters(in: .whitespaces)

        // vars.* -> runtime variable
        if trimmed.hasPrefix("vars.") {
            let varName = String(trimmed.dropFirst(5))
            return context.variables[varName] ?? .null
        }

        // artifact.field -> artifact metadata
        if trimmed.contains(".") && !trimmed.hasPrefix("vars.") {
            let parts = trimmed.split(separator: ".", maxSplits: 1).map(String.init)
            if parts.count == 2 {
                let artifactName = parts[0]
                let fieldName = parts[1]
                if let value = context.artifactFields[artifactName]?[fieldName] {
                    return value
                }
                if artifactName == "implementation_self_assessment_v2",
                   let value = context.artifactFields["implementation_self_assessment"]?[fieldName] {
                    return value
                }
                return .null
            }
        }

        // Literal: true/false
        if trimmed == "true" { return .bool(true) }
        if trimmed == "false" { return .bool(false) }

        // Literal: integer
        if let intVal = Int(trimmed) {
            return .int(intVal)
        }

        // Literal: double
        if let doubleVal = Double(trimmed) {
            return .double(doubleVal)
        }

        // Literal: quoted string
        if (trimmed.hasPrefix("'") && trimmed.hasSuffix("'"))
            || (trimmed.hasPrefix("\"") && trimmed.hasSuffix("\"")) {
            return .string(String(trimmed.dropFirst().dropLast()))
        }

        // Bare string
        return .string(trimmed)
    }

    // MARK: - Operator Application

    private static func applyOperator(
        _ lhs: AnyCodableValue,
        _ op: ComparisonOp,
        _ rhs: AnyCodableValue
    ) -> Bool {
        switch op {
        case .equal:
            return valuesEqual(lhs, rhs)
        case .notEqual:
            return !valuesEqual(lhs, rhs)
        case .lessThan:
            return compareNumeric(lhs, rhs) == .orderedAscending
        case .lessOrEqual:
            let cmp = compareNumeric(lhs, rhs)
            return cmp == .orderedAscending || cmp == .orderedSame
        case .greaterThan:
            return compareNumeric(lhs, rhs) == .orderedDescending
        case .greaterOrEqual:
            let cmp = compareNumeric(lhs, rhs)
            return cmp == .orderedDescending || cmp == .orderedSame
        }
    }

    private static func valuesEqual(_ a: AnyCodableValue, _ b: AnyCodableValue) -> Bool {
        switch (a, b) {
        case (.string(let l), .string(let r)): return l == r
        case (.int(let l), .int(let r)): return l == r
        case (.double(let l), .double(let r)): return l == r
        case (.bool(let l), .bool(let r)): return l == r
        case (.int(let l), .double(let r)): return Double(l) == r
        case (.double(let l), .int(let r)): return l == Double(r)
        case (.null, .null): return true
        default: return false
        }
    }

    private static func compareNumeric(
        _ a: AnyCodableValue,
        _ b: AnyCodableValue
    ) -> ComparisonResult {
        let aDouble = toDouble(a)
        let bDouble = toDouble(b)
        guard let aVal = aDouble, let bVal = bDouble else {
            return .orderedSame // non-numeric comparison defaults to equal (fail safe)
        }
        if aVal > bVal { return .orderedDescending }
        if aVal < bVal { return .orderedAscending }
        return .orderedSame
    }

    private static func toDouble(_ value: AnyCodableValue) -> Double? {
        switch value {
        case .int(let v): return Double(v)
        case .double(let v): return v
        case .string(let v): return Double(v)
        default: return nil
        }
    }
}
