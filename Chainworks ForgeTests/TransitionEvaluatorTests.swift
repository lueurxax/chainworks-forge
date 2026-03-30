import Testing
@testable import Chainworks_Forge

// MARK: - Parameterized test argument types

/// Argument type for artifact-exists parameterized tests.
struct ArtifactExistsCase: Sendable {
    let label: String
    let artifacts: Set<String>
    let queryArtifact: String
    let expected: Bool
}

/// Argument type for expression literal tests.
struct ExpressionLiteralCase: Sendable {
    let label: String
    let expression: String
    let expected: Bool
}

/// Argument type for expression-exists tests.
struct ExpressionExistsCase: Sendable {
    let label: String
    let artifacts: Set<String>
    let expression: String
    let expected: Bool
}

/// Argument type for vars comparison tests.
struct VarsComparisonCase: Sendable {
    let label: String
    let variableKey: String
    let variableValue: AnyCodableValue
    let expression: String
    let expected: Bool
}

/// Argument type for artifact field comparison tests.
struct ArtifactFieldCase: Sendable {
    let label: String
    let artifactName: String
    let fieldName: String
    let fieldValue: AnyCodableValue
    let expression: String
    let expected: Bool
}

/// Argument type for and/or expression tests.
struct LogicalExpressionCase: Sendable {
    let label: String
    let artifacts: Set<String>
    let variables: [String: AnyCodableValue]
    let expression: String
    let expected: Bool
}

/// Argument type for edge-case tests.
struct EdgeCase: Sendable {
    let label: String
    let variables: [String: AnyCodableValue]
    let expression: String
    let expected: Bool
}

@Suite("TransitionEvaluator")
struct TransitionEvaluatorTests {

    // MARK: - Helpers

    private func makeContext(
        artifacts: Set<String> = [],
        approvalGranted: Bool = false,
        variables: [String: AnyCodableValue] = [:],
        artifactFields: [String: [String: AnyCodableValue]] = [:]
    ) -> TransitionEvaluator.EvaluationContext {
        TransitionEvaluator.EvaluationContext(
            producedArtifactNames: artifacts,
            approvalGranted: approvalGranted,
            variables: variables,
            artifactFields: artifactFields
        )
    }

    // MARK: - always

    @Test("always returns true")
    func alwaysReturnsTrue() {
        let ctx = makeContext()
        #expect(TransitionEvaluator.evaluate(.always, context: ctx))
    }

    // MARK: - artifactExists (parameterized)

    @Test("artifact exists", arguments: [
        ArtifactExistsCase(label: "present", artifacts: ["idea_brief", "proposal_current"], queryArtifact: "idea_brief", expected: true),
        ArtifactExistsCase(label: "missing", artifacts: ["proposal_current"], queryArtifact: "idea_brief", expected: false)
    ])
    func artifactExists(testCase: ArtifactExistsCase) {
        let ctx = makeContext(artifacts: testCase.artifacts)
        let result = TransitionEvaluator.evaluate(.artifactExists(testCase.queryArtifact), context: ctx)
        #expect(result == testCase.expected)
    }

    // MARK: - approvalGranted (parameterized)

    @Test("approval granted", arguments: [true, false])
    func approvalGranted(granted: Bool) {
        let ctx = makeContext(approvalGranted: granted)
        let result = TransitionEvaluator.evaluate(.approvalGranted, context: ctx)
        #expect(result == granted)
    }

    // MARK: - Expression: true/false literals (parameterized)

    @Test("expression literals", arguments: [
        ExpressionLiteralCase(label: "true string", expression: "true", expected: true),
        ExpressionLiteralCase(label: "quoted true", expression: "'true'", expected: true),
        ExpressionLiteralCase(label: "false string", expression: "false", expected: false)
    ])
    func expressionLiterals(testCase: ExpressionLiteralCase) {
        let ctx = makeContext()
        let result = TransitionEvaluator.evaluate(.expression(testCase.expression), context: ctx)
        #expect(result == testCase.expected)
    }

    // MARK: - Expression: exists() (parameterized)

    @Test("expression exists", arguments: [
        ExpressionExistsCase(label: "present", artifacts: ["proposal_current"], expression: "exists('proposal_current')", expected: true),
        ExpressionExistsCase(label: "missing", artifacts: [], expression: "exists('proposal_current')", expected: false)
    ])
    func expressionExists(testCase: ExpressionExistsCase) {
        let ctx = makeContext(artifacts: testCase.artifacts)
        let result = TransitionEvaluator.evaluate(.expression(testCase.expression), context: ctx)
        #expect(result == testCase.expected)
    }

    // MARK: - Expression: approval.granted (parameterized)

    @Test("expression approval.granted", arguments: [
        (true, true),
        (false, false)
    ])
    func expressionApprovalGranted(granted: Bool, expected: Bool) {
        let ctx = makeContext(approvalGranted: granted)
        let result = TransitionEvaluator.evaluate(.expression("approval.granted == true"), context: ctx)
        #expect(result == expected)
    }

    // MARK: - Expression: vars comparison (parameterized)

    @Test("vars comparison", arguments: [
        VarsComparisonCase(label: "equal int", variableKey: "revision_count", variableValue: .int(3), expression: "vars.revision_count == 3", expected: true),
        VarsComparisonCase(label: "greater than", variableKey: "revision_count", variableValue: .int(5), expression: "vars.revision_count > 3", expected: true),
        VarsComparisonCase(label: "greater or equal", variableKey: "revision_count", variableValue: .int(3), expression: "vars.revision_count >= 3", expected: true),
        VarsComparisonCase(label: "less than fails", variableKey: "revision_count", variableValue: .int(2), expression: "vars.revision_count > 3", expected: false)
    ])
    func varsComparison(testCase: VarsComparisonCase) {
        let ctx = makeContext(variables: [testCase.variableKey: testCase.variableValue])
        let result = TransitionEvaluator.evaluate(.expression(testCase.expression), context: ctx)
        #expect(result == testCase.expected)
    }

    // MARK: - Expression: artifact field comparison (parameterized)

    @Test("artifact field comparison", arguments: [
        ArtifactFieldCase(label: "int score >= 80", artifactName: "review_report", fieldName: "score", fieldValue: .int(85), expression: "review_report.score >= 80", expected: true),
        ArtifactFieldCase(label: "string status == pass", artifactName: "review_report", fieldName: "status", fieldValue: .string("pass"), expression: "review_report.status == 'pass'", expected: true),
        ArtifactFieldCase(label: "double score <= target", artifactName: "proposal_review_summary", fieldName: "aggregate_score", fieldValue: .double(9.0), expression: "proposal_review_summary.aggregate_score <= 9.1", expected: true),
        ArtifactFieldCase(label: "string status != implemented", artifactName: "audit_report", fieldName: "status", fieldValue: .string("Needs Work"), expression: "audit_report.status != 'Implemented'", expected: true)
    ])
    func artifactFieldComparison(testCase: ArtifactFieldCase) {
        let ctx = makeContext(
            artifactFields: [testCase.artifactName: [testCase.fieldName: testCase.fieldValue]]
        )
        let result = TransitionEvaluator.evaluate(.expression(testCase.expression), context: ctx)
        #expect(result == testCase.expected)
    }

    // MARK: - Expression: and / or (parameterized)

    @Test("logical and/or expressions", arguments: [
        LogicalExpressionCase(label: "and both true", artifacts: ["proposal_current"], variables: ["count": .int(5)], expression: "exists('proposal_current') and vars.count > 3", expected: true),
        LogicalExpressionCase(label: "and one false", artifacts: [], variables: ["count": .int(5)], expression: "exists('proposal_current') and vars.count > 3", expected: false),
        LogicalExpressionCase(label: "or one true", artifacts: ["proposal_current"], variables: ["count": .int(1)], expression: "exists('proposal_current') or vars.count > 3", expected: true),
        LogicalExpressionCase(label: "or both false", artifacts: [], variables: ["count": .int(1)], expression: "exists('proposal_current') or vars.count > 3", expected: false)
    ])
    func logicalExpressions(testCase: LogicalExpressionCase) {
        let ctx = makeContext(
            artifacts: testCase.artifacts,
            variables: testCase.variables
        )
        let result = TransitionEvaluator.evaluate(.expression(testCase.expression), context: ctx)
        #expect(result == testCase.expected)
    }

    // MARK: - evaluateFirst

    @Test("evaluateFirst picks correct transition")
    func evaluateFirstPicksCorrectTransition() {
        let transitions = [
            ExecutableTransition(to: "state_a", condition: .artifactExists("missing")),
            ExecutableTransition(to: "state_b", condition: .artifactExists("idea_brief")),
            ExecutableTransition(to: "state_c", condition: .always),
        ]
        let ctx = makeContext(artifacts: ["idea_brief"])
        let result = TransitionEvaluator.evaluateFirst(transitions: transitions, context: ctx)
        #expect(result?.to == "state_b")
    }

    @Test("evaluateFirst returns nil when none match")
    func evaluateFirstReturnsNilWhenNoneMatch() {
        let transitions = [
            ExecutableTransition(to: "state_a", condition: .artifactExists("missing")),
        ]
        let ctx = makeContext(artifacts: [])
        let result = TransitionEvaluator.evaluateFirst(transitions: transitions, context: ctx)
        #expect(result == nil)
    }

    // MARK: - Edge cases (parameterized)

    @Test("edge cases", arguments: [
        EdgeCase(label: "unrecognized expression returns false", variables: [:], expression: "some.random.nonsense", expected: false),
        EdgeCase(label: "missing var returns false", variables: [:], expression: "vars.missing_var == 42", expected: false),
        EdgeCase(label: "int compared to double", variables: ["val": .double(3.0)], expression: "vars.val == 3", expected: true)
    ])
    func edgeCases(testCase: EdgeCase) {
        let ctx = makeContext(variables: testCase.variables)
        let result = TransitionEvaluator.evaluate(.expression(testCase.expression), context: ctx)
        #expect(result == testCase.expected)
    }
}
