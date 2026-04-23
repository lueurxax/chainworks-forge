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
    struct AuthorityResolution: Sendable {
        let selectedTransition: ExecutableTransition?
        let selectedEvaluation: CandidateTransitionEvaluation?
        let candidateEvaluations: [CandidateTransitionEvaluation]
        let conflictReason: WorkflowConflictReason?
        let operatorLabel: String?

        var selectedNextStateID: String? {
            selectedTransition?.to
        }
    }

    /// Context provided to the evaluator at runtime.
    struct EvaluationContext: Sendable {
        /// Names of artifacts that have been produced so far in this run.
        let producedArtifactNames: Set<String>
        /// Names declared by the workflow/catalog artifact contract. Nil preserves
        /// legacy Boolean evaluation where undeclared and absent both fail closed.
        let declaredArtifactNames: Set<String>?
        /// Whether approval has been granted for the current stage.
        let approvalGranted: Bool
        /// Whether approval has been rejected for the current stage.
        let approvalRejected: Bool
        /// Runtime variables (from RunPlan.variables, may be mutated by loop counters).
        let variables: [String: AnyCodableValue]
        /// Artifact metadata for field-level expression checks.
        /// Key: artifact name, Value: dictionary of field values.
        let artifactFields: [String: [String: AnyCodableValue]]

        init(
            producedArtifactNames: Set<String>,
            declaredArtifactNames: Set<String>? = nil,
            approvalGranted: Bool,
            approvalRejected: Bool,
            variables: [String: AnyCodableValue],
            artifactFields: [String: [String: AnyCodableValue]]
        ) {
            self.producedArtifactNames = producedArtifactNames
            self.declaredArtifactNames = declaredArtifactNames
            self.approvalGranted = approvalGranted
            self.approvalRejected = approvalRejected
            self.variables = variables
            self.artifactFields = artifactFields
        }
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

    /// Proposal 017: Evaluate every transition candidate with typed diagnostic
    /// output while preserving the legacy Boolean evaluator above.
    static func evaluateCandidates(
        transitions: [ExecutableTransition],
        fromStateID: String,
        context: EvaluationContext
    ) -> [CandidateTransitionEvaluation] {
        transitions.enumerated().map { index, transition in
            let classification = classify(transition.condition, context: context)
            return CandidateTransitionEvaluation(
                transitionID: "\(fromStateID)->\(transition.to)#\(index)",
                fromStateID: fromStateID,
                toStateID: transition.to,
                conditionExpressionID: transition.condition.expressionID,
                result: classification.result,
                requiredArtifacts: classification.requiredArtifacts,
                missingArtifacts: classification.missingArtifacts,
                missingFields: classification.missingFields,
                sourceArtifactIDs: classification.sourceArtifactIDs,
                sourceAgentExecutionID: nil,
                sanitizedDiagnostic: classification.sanitizedDiagnostic
            )
        }
    }

    static func resolveAuthority(
        transitions: [ExecutableTransition],
        fromStateID: String,
        context: EvaluationContext
    ) -> AuthorityResolution {
        let evaluations = evaluateCandidates(
            transitions: transitions,
            fromStateID: fromStateID,
            context: context
        )
        let matchedIndexes = evaluations.indices.filter { evaluations[$0].result == .matched }

        if matchedIndexes.count == 1, let selectedIndex = matchedIndexes.first {
            return AuthorityResolution(
                selectedTransition: transitions[selectedIndex],
                selectedEvaluation: evaluations[selectedIndex],
                candidateEvaluations: evaluations,
                conflictReason: nil,
                operatorLabel: nil
            )
        }

        if matchedIndexes.count > 1 {
            return AuthorityResolution(
                selectedTransition: nil,
                selectedEvaluation: nil,
                candidateEvaluations: evaluations,
                conflictReason: .multipleDeclarativeTransitionsMatchedWithoutTieBreak,
                operatorLabel: "Multiple declarative transitions matched from '\(fromStateID)'"
            )
        }

        if evaluations.contains(where: {
            $0.result == .invalidExpression || $0.result == .evaluationError
        }) {
            return AuthorityResolution(
                selectedTransition: nil,
                selectedEvaluation: nil,
                candidateEvaluations: evaluations,
                conflictReason: .workflowConflictUnverifiable,
                operatorLabel: "Transition conditions from '\(fromStateID)' could not be verified"
            )
        }

        if evaluations.contains(where: { $0.result == .missingInput }) {
            return AuthorityResolution(
                selectedTransition: nil,
                selectedEvaluation: nil,
                candidateEvaluations: evaluations,
                conflictReason: .requiredArtifactOrFieldMissingForTransition,
                operatorLabel: "A required artifact or field is missing for transition out of '\(fromStateID)'"
            )
        }

        return AuthorityResolution(
            selectedTransition: nil,
            selectedEvaluation: nil,
            candidateEvaluations: evaluations,
            conflictReason: .noDeclarativeTransitionMatched,
            operatorLabel: "No declarative transition matched from '\(fromStateID)'"
        )
    }

    private struct CandidateClassification {
        let result: CandidateTransitionResult
        let requiredArtifacts: [String]
        let missingArtifacts: [String]
        let missingFields: [String]
        let sourceArtifactIDs: [String]
        let sanitizedDiagnostic: String?
    }

    private static func classify(
        _ condition: TransitionCondition,
        context: EvaluationContext
    ) -> CandidateClassification {
        switch condition {
        case .always:
            return CandidateClassification(
                result: .matched,
                requiredArtifacts: [],
                missingArtifacts: [],
                missingFields: [],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: nil
            )
        case .artifactExists(let name):
            return classifyArtifactPresence(name, context: context)
        case .approvalGranted:
            return CandidateClassification(
                result: context.approvalGranted ? .matched : .notMatched,
                requiredArtifacts: [],
                missingArtifacts: [],
                missingFields: [],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: nil
            )
        case .approvalRejected:
            return CandidateClassification(
                result: context.approvalRejected ? .matched : .notMatched,
                requiredArtifacts: [],
                missingArtifacts: [],
                missingFields: [],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: nil
            )
        case .expression(let expr):
            return classifyExpression(expr, context: context)
        }
    }

    private static func classifyExpression(
        _ expr: String,
        context: EvaluationContext
    ) -> CandidateClassification {
        let trimmed = expr.trimmingCharacters(in: .whitespaces)

        if trimmed == "true" || trimmed == "'true'" {
            return matchedClassification()
        }
        if trimmed == "false" || trimmed == "'false'" {
            return notMatchedClassification()
        }

        if let andResult = splitConnective(trimmed, connective: " and ") {
            return combineClassifications(
                classifyExpression(andResult.lhs, context: context),
                classifyExpression(andResult.rhs, context: context),
                matchedWhen: { $0 == .matched && $1 == .matched }
            )
        }
        if let orResult = splitConnective(trimmed, connective: " or ") {
            return combineClassifications(
                classifyExpression(orResult.lhs, context: context),
                classifyExpression(orResult.rhs, context: context),
                matchedWhen: { $0 == .matched || $1 == .matched }
            )
        }

        if trimmed.hasPrefix("exists(") && trimmed.hasSuffix(")") {
            let inner = String(trimmed.dropFirst(7).dropLast(1))
                .trimmingCharacters(in: CharacterSet(charactersIn: "'\""))
            return classifyArtifactPresence(inner, context: context)
        }

        if trimmed == "approval.granted == true" {
            return context.approvalGranted ? matchedClassification() : notMatchedClassification()
        }
        if trimmed == "approval.rejected == true" {
            return context.approvalRejected ? matchedClassification() : notMatchedClassification()
        }

        if let comparison = parseComparison(trimmed) {
            if let artifactRef = artifactFieldReference(comparison.lhs) {
                return classifyArtifactFieldComparison(
                    artifactName: artifactRef.artifact,
                    fieldName: artifactRef.field,
                    comparison: comparison,
                    context: context
                )
            }
            return evaluate(conditionResult: evaluateExpression(trimmed, context: context))
        }

        return CandidateClassification(
            result: .invalidExpression,
            requiredArtifacts: [],
            missingArtifacts: [],
            missingFields: [],
            sourceArtifactIDs: [],
            sanitizedDiagnostic: "Unsupported transition condition: \(trimmed)"
        )
    }

    private static func classifyArtifactPresence(
        _ artifactName: String,
        context: EvaluationContext
    ) -> CandidateClassification {
        if !isDeclaredArtifact(artifactName, context: context) {
            return CandidateClassification(
                result: .invalidExpression,
                requiredArtifacts: [artifactName],
                missingArtifacts: [],
                missingFields: [],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: "Artifact \(artifactName) is not declared by the workflow/catalog contract"
            )
        }

        if isProducedArtifact(artifactName, context: context) {
            return CandidateClassification(
                result: .matched,
                requiredArtifacts: [artifactName],
                missingArtifacts: [],
                missingFields: [],
                sourceArtifactIDs: [artifactName],
                sanitizedDiagnostic: nil
            )
        }

        return CandidateClassification(
            result: .missingInput,
            requiredArtifacts: [artifactName],
            missingArtifacts: [artifactName],
            missingFields: [],
            sourceArtifactIDs: [],
            sanitizedDiagnostic: "Declared artifact \(artifactName) is absent"
        )
    }

    private static func classifyArtifactFieldComparison(
        artifactName: String,
        fieldName: String,
        comparison: Comparison,
        context: EvaluationContext
    ) -> CandidateClassification {
        let fieldRef = "\(artifactName).\(fieldName)"
        if !isDeclaredArtifact(artifactName, context: context) {
            return CandidateClassification(
                result: .invalidExpression,
                requiredArtifacts: [artifactName],
                missingArtifacts: [],
                missingFields: [],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: "Artifact field \(fieldRef) references an undeclared artifact"
            )
        }

        guard isProducedArtifact(artifactName, context: context) else {
            return CandidateClassification(
                result: .missingInput,
                requiredArtifacts: [artifactName],
                missingArtifacts: [artifactName],
                missingFields: [],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: "Declared artifact \(artifactName) is absent"
            )
        }

        guard context.artifactFields[artifactName]?[fieldName] != nil else {
            return CandidateClassification(
                result: .missingInput,
                requiredArtifacts: [artifactName],
                missingArtifacts: [],
                missingFields: [fieldRef],
                sourceArtifactIDs: [],
                sanitizedDiagnostic: "Declared artifact field \(fieldRef) is absent"
            )
        }

        return CandidateClassification(
            result: evaluateExpression(
                "\(comparison.lhs) \(comparison.op.sourceToken) \(comparison.rhs)",
                context: context
            ) ? .matched : .notMatched,
            requiredArtifacts: [artifactName],
            missingArtifacts: [],
            missingFields: [],
            sourceArtifactIDs: [artifactName],
            sanitizedDiagnostic: nil
        )
    }

    private static func combineClassifications(
        _ left: CandidateClassification,
        _ right: CandidateClassification,
        matchedWhen: (CandidateTransitionResult, CandidateTransitionResult) -> Bool
    ) -> CandidateClassification {
        let dominantResult = [
            CandidateTransitionResult.evaluationError,
            .invalidExpression,
            .missingInput
        ].first { result in
            left.result == result || right.result == result
        }
        let result = dominantResult ?? (matchedWhen(left.result, right.result) ? .matched : .notMatched)
        return CandidateClassification(
            result: result,
            requiredArtifacts: unique(left.requiredArtifacts + right.requiredArtifacts),
            missingArtifacts: unique(left.missingArtifacts + right.missingArtifacts),
            missingFields: unique(left.missingFields + right.missingFields),
            sourceArtifactIDs: unique(left.sourceArtifactIDs + right.sourceArtifactIDs),
            sanitizedDiagnostic: left.sanitizedDiagnostic ?? right.sanitizedDiagnostic
        )
    }

    private static func evaluate(conditionResult: Bool) -> CandidateClassification {
        conditionResult ? matchedClassification() : notMatchedClassification()
    }

    private static func matchedClassification() -> CandidateClassification {
        CandidateClassification(
            result: .matched,
            requiredArtifacts: [],
            missingArtifacts: [],
            missingFields: [],
            sourceArtifactIDs: [],
            sanitizedDiagnostic: nil
        )
    }

    private static func notMatchedClassification() -> CandidateClassification {
        CandidateClassification(
            result: .notMatched,
            requiredArtifacts: [],
            missingArtifacts: [],
            missingFields: [],
            sourceArtifactIDs: [],
            sanitizedDiagnostic: nil
        )
    }

    private static func isDeclaredArtifact(
        _ artifactName: String,
        context: EvaluationContext
    ) -> Bool {
        context.declaredArtifactNames?.contains(artifactName) ?? true
    }

    private static func isProducedArtifact(
        _ artifactName: String,
        context: EvaluationContext
    ) -> Bool {
        context.producedArtifactNames.contains(artifactName)
            || context.artifactFields[artifactName] != nil
    }

    private struct ArtifactFieldReference {
        let artifact: String
        let field: String
    }

    private static func artifactFieldReference(_ ref: String) -> ArtifactFieldReference? {
        let trimmed = ref.trimmingCharacters(in: .whitespaces)
        guard trimmed.contains("."), !trimmed.hasPrefix("vars.") else {
            return nil
        }
        let parts = trimmed.split(separator: ".", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return nil }
        return ArtifactFieldReference(artifact: parts[0], field: parts[1])
    }

    private static func unique(_ values: [String]) -> [String] {
        var seen = Set<String>()
        var result: [String] = []
        for value in values where !seen.contains(value) {
            seen.insert(value)
            result.append(value)
        }
        return result
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

        var sourceToken: String {
            switch self {
            case .equal: return "=="
            case .notEqual: return "!="
            case .lessThan: return "<"
            case .lessOrEqual: return "<="
            case .greaterThan: return ">"
            case .greaterOrEqual: return ">="
            }
        }
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

private extension TransitionCondition {
    var expressionID: String? {
        switch self {
        case .always:
            return "true"
        case .artifactExists(let name):
            return "exists('\(name)')"
        case .approvalGranted:
            return "approval.granted == true"
        case .approvalRejected:
            return "approval.rejected == true"
        case .expression(let expression):
            return expression
        }
    }
}
