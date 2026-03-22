import XCTest
@testable import Chainworks_Forge

final class TransitionEvaluatorTests: XCTestCase {

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

    func testAlwaysReturnsTrue() {
        let ctx = makeContext()
        XCTAssertTrue(TransitionEvaluator.evaluate(.always, context: ctx))
    }

    // MARK: - artifactExists

    func testArtifactExistsWhenPresent() {
        let ctx = makeContext(artifacts: ["idea_brief", "proposal_current"])
        XCTAssertTrue(TransitionEvaluator.evaluate(.artifactExists("idea_brief"), context: ctx))
    }

    func testArtifactExistsWhenMissing() {
        let ctx = makeContext(artifacts: ["proposal_current"])
        XCTAssertFalse(TransitionEvaluator.evaluate(.artifactExists("idea_brief"), context: ctx))
    }

    // MARK: - approvalGranted

    func testApprovalGrantedTrue() {
        let ctx = makeContext(approvalGranted: true)
        XCTAssertTrue(TransitionEvaluator.evaluate(.approvalGranted, context: ctx))
    }

    func testApprovalGrantedFalse() {
        let ctx = makeContext(approvalGranted: false)
        XCTAssertFalse(TransitionEvaluator.evaluate(.approvalGranted, context: ctx))
    }

    // MARK: - Expression: true/false literals

    func testExpressionTrueString() {
        let ctx = makeContext()
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("true"), context: ctx))
    }

    func testExpressionQuotedTrue() {
        let ctx = makeContext()
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("'true'"), context: ctx))
    }

    func testExpressionFalseString() {
        let ctx = makeContext()
        XCTAssertFalse(TransitionEvaluator.evaluate(.expression("false"), context: ctx))
    }

    // MARK: - Expression: exists()

    func testExpressionExistsPresent() {
        let ctx = makeContext(artifacts: ["proposal_current"])
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("exists('proposal_current')"), context: ctx))
    }

    func testExpressionExistsMissing() {
        let ctx = makeContext(artifacts: [])
        XCTAssertFalse(TransitionEvaluator.evaluate(.expression("exists('proposal_current')"), context: ctx))
    }

    // MARK: - Expression: approval.granted

    func testExpressionApprovalGranted() {
        let ctx = makeContext(approvalGranted: true)
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("approval.granted == true"), context: ctx))
    }

    func testExpressionApprovalNotGranted() {
        let ctx = makeContext(approvalGranted: false)
        XCTAssertFalse(TransitionEvaluator.evaluate(.expression("approval.granted == true"), context: ctx))
    }

    // MARK: - Expression: vars comparison

    func testVarsEqualInt() {
        let ctx = makeContext(variables: ["revision_count": .int(3)])
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("vars.revision_count == 3"), context: ctx))
    }

    func testVarsGreaterThan() {
        let ctx = makeContext(variables: ["revision_count": .int(5)])
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("vars.revision_count > 3"), context: ctx))
    }

    func testVarsGreaterOrEqual() {
        let ctx = makeContext(variables: ["revision_count": .int(3)])
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("vars.revision_count >= 3"), context: ctx))
    }

    func testVarsLessThanFails() {
        let ctx = makeContext(variables: ["revision_count": .int(2)])
        XCTAssertFalse(TransitionEvaluator.evaluate(.expression("vars.revision_count > 3"), context: ctx))
    }

    // MARK: - Expression: artifact field comparison

    func testArtifactFieldEqual() {
        let ctx = makeContext(
            artifactFields: ["review_report": ["score": .int(85)]]
        )
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("review_report.score >= 80"), context: ctx))
    }

    func testArtifactFieldEqualString() {
        let ctx = makeContext(
            artifactFields: ["review_report": ["status": .string("pass")]]
        )
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("review_report.status == 'pass'"), context: ctx))
    }

    // MARK: - Expression: and / or

    func testAndBothTrue() {
        let ctx = makeContext(
            artifacts: ["proposal_current"],
            variables: ["count": .int(5)]
        )
        XCTAssertTrue(TransitionEvaluator.evaluate(
            .expression("exists('proposal_current') and vars.count > 3"),
            context: ctx
        ))
    }

    func testAndOneFalse() {
        let ctx = makeContext(
            artifacts: [],
            variables: ["count": .int(5)]
        )
        XCTAssertFalse(TransitionEvaluator.evaluate(
            .expression("exists('proposal_current') and vars.count > 3"),
            context: ctx
        ))
    }

    func testOrOneTrue() {
        let ctx = makeContext(
            artifacts: ["proposal_current"],
            variables: ["count": .int(1)]
        )
        XCTAssertTrue(TransitionEvaluator.evaluate(
            .expression("exists('proposal_current') or vars.count > 3"),
            context: ctx
        ))
    }

    func testOrBothFalse() {
        let ctx = makeContext(
            artifacts: [],
            variables: ["count": .int(1)]
        )
        XCTAssertFalse(TransitionEvaluator.evaluate(
            .expression("exists('proposal_current') or vars.count > 3"),
            context: ctx
        ))
    }

    // MARK: - evaluateFirst

    func testEvaluateFirstPicksCorrectTransition() {
        let transitions = [
            ExecutableTransition(to: "state_a", condition: .artifactExists("missing")),
            ExecutableTransition(to: "state_b", condition: .artifactExists("idea_brief")),
            ExecutableTransition(to: "state_c", condition: .always),
        ]
        let ctx = makeContext(artifacts: ["idea_brief"])
        let result = TransitionEvaluator.evaluateFirst(transitions: transitions, context: ctx)
        XCTAssertEqual(result?.to, "state_b")
    }

    func testEvaluateFirstReturnsNilWhenNoneMatch() {
        let transitions = [
            ExecutableTransition(to: "state_a", condition: .artifactExists("missing")),
        ]
        let ctx = makeContext(artifacts: [])
        let result = TransitionEvaluator.evaluateFirst(transitions: transitions, context: ctx)
        XCTAssertNil(result)
    }

    // MARK: - Unrecognized expression fails closed

    func testUnrecognizedExpressionReturnsFalse() {
        let ctx = makeContext()
        XCTAssertFalse(TransitionEvaluator.evaluate(.expression("some.random.nonsense"), context: ctx))
    }

    // MARK: - Edge cases

    func testVarsMissing() {
        let ctx = makeContext(variables: [:])
        XCTAssertFalse(TransitionEvaluator.evaluate(.expression("vars.missing_var == 42"), context: ctx))
    }

    func testCompareIntToDouble() {
        let ctx = makeContext(variables: ["val": .double(3.0)])
        XCTAssertTrue(TransitionEvaluator.evaluate(.expression("vars.val == 3"), context: ctx))
    }
}
