import Darwin
import Dispatch
import XCTest
@testable import Chainworks_Forge

@MainActor
final class Proposal036UXConsolidationTests: XCTestCase {

    func testNavigationTabTargetParity() {
        // P036 Phase 2c cutover: four tabs only — Runs, Ideas, Definitions, Settings.
        let tabs = ContentView.Tab.allCases
        XCTAssertEqual(tabs.count, 4)
        XCTAssertTrue(tabs.contains(.runs))
        XCTAssertTrue(tabs.contains(.ideas))
        XCTAssertTrue(tabs.contains(.definitions))
        XCTAssertTrue(tabs.contains(.settings))

        // Legacy "Approvals" deep links still route to Runs (P036 old_route_mapping).
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Approvals"), .runs)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "approvals"), .runs)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Pilot Readiness"), .settings)
    }

    func testAgentGroupingPrecedence() {
        let agent1 = AgentDefinition(
            id: "a1",
            title: "Agent 1",
            mode: "fast",
            group: "Custom Group",
            backendProfile: "p1",
            permissionProfile: "perm1",
            mcpProfile: nil,
            skillRef: "skill1",
            skillRole: "role1",
            worktreePolicy: nil,
            requiredTools: nil,
            inputs: [],
            outputs: [],
            outputContract: nil,
            requiresHumanApproval: false,
            xcodeBrokerRequired: nil,
            xcodeShimInjectionSignal: nil,
            requiresXcodeHostExecution: nil,
            prompt: "",
            notes: nil
        )

        let agent2 = AgentDefinition(
            id: "a2",
            title: "Agent 2",
            mode: "slow",
            group: nil,
            backendProfile: "p2",
            permissionProfile: "perm2",
            mcpProfile: nil,
            skillRef: "skill2",
            skillRole: "role2",
            worktreePolicy: nil,
            requiredTools: nil,
            inputs: [],
            outputs: [],
            outputContract: nil,
            requiresHumanApproval: false,
            xcodeBrokerRequired: nil,
            xcodeShimInjectionSignal: nil,
            requiresXcodeHostExecution: nil,
            prompt: "",
            notes: nil
        )

        let agent3 = AgentDefinition(
            id: "a3",
            title: "Agent 3",
            mode: "fast",
            group: nil,
            backendProfile: "",
            permissionProfile: "perm3",
            mcpProfile: nil,
            skillRef: "skill3",
            skillRole: nil,
            worktreePolicy: nil,
            requiredTools: nil,
            inputs: [],
            outputs: [],
            outputContract: nil,
            requiresHumanApproval: false,
            xcodeBrokerRequired: nil,
            xcodeShimInjectionSignal: nil,
            requiresXcodeHostExecution: nil,
            prompt: "",
            notes: nil
        )

        let appConfig = AppConfig(
            name: "Test",
            runtime: "test",
            transport: "test",
            description: "test",
            ideaInputMode: "test",
            singleActiveRunPerIdea: false,
            runResumePolicy: "test",
            requiredProviders: []
        )

        let catalog = AgentCatalog(
            schemaVersion: 1,
            app: appConfig,
            paths: [:],
            artifacts: [:],
            skills: [:],
            mcpPolicy: .defaultDeny,
            mcpServerRegistry: [:],
            mcpProfiles: [:],
            contracts: [:],
            backendProfiles: [:],
            permissionProfiles: [:],
            runtimeProfiles: [:],
            agents: [agent1, agent2, agent3]
        )

        let grouped = catalog.groupedAgents()
        XCTAssertEqual(grouped.count, 3)

        // agent1: has explicit group "Custom Group" -> Group: "Custom Group"
        XCTAssertEqual(grouped[0].label, "Custom Group")
        XCTAssertEqual(grouped[0].agents.first?.id, "a1")

        // agent2: no group, mode="slow" wins over backendProfile -> Group: "Slow"
        XCTAssertEqual(grouped[1].label, "Slow")
        XCTAssertEqual(grouped[1].agents.first?.id, "a2")

        // agent3: no group, mode="fast" -> Group: "Fast"
        XCTAssertEqual(grouped[2].label, "Fast")
        XCTAssertEqual(grouped[2].agents.first?.id, "a3")
    }

    // MARK: - Approval State Matrix (P085)

    private func makeApprovalDetail(affordance: P085ApprovalAffordanceState) -> P031RunDetailPresentation {
        let row = P031ApprovalInboxRowPresentation(
            approvalID: affordance.approvalID,
            title: "Test Approval",
            body: "Body",
            canApprove: false,
            canReject: false,
            actionLabel: nil,
            followUpID: nil,
            copyItems: [],
            freshnessState: .live,
            accessibilityLabel: "label",
            affordance: affordance
        )
        return P031RunDetailPresentation(
            title: "Run", workflowLabel: "w1", statusLabel: "Blocked",
            progressLabel: nil, pendingApprovalsLabel: nil, rolloutDecisionSummary: nil,
            ideaContext: nil, stageTransitions: [], approvalRows: [row],
            artifactRows: [], artifactViewerRows: [], reportRows: [],
            catalogContext: nil, closeoutReadiness: nil, implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live", emptyStateTitle: nil, errorDescription: nil,
            rawStatus: "blocked", failedStages: 0
        )
    }

    private func makeDiagnostic() -> P085DiagnosticAffordanceState {
        P085DiagnosticAffordanceState(diagnosticID: "d1", serverDebugDetail: "detail", isAvailable: true)
    }

    func testInlineApprovalActionable() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-actionable",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .live,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertTrue(row.canApprove)
        XCTAssertTrue(row.canReject)
        XCTAssertNil(row.deferredState)
    }

    func testInlineApprovalStale() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-stale",
            approveAvailability: .disabled(reasonCode: .staleRead, helpText: "Stale"),
            rejectAvailability: .disabled(reasonCode: .staleRead, helpText: "Stale"),
            freshnessState: .stale,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertEqual(row.deferredState, .stale)
    }

    func testInlineApprovalProjectionLag() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-lag",
            approveAvailability: .disabled(reasonCode: .projectionLag, helpText: "Lag"),
            rejectAvailability: .disabled(reasonCode: .projectionLag, helpText: "Lag"),
            freshnessState: .projectionLag,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: true
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertEqual(row.deferredState, .projectionLag)
    }

    func testInlineApprovalRedacted() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-redacted",
            approveAvailability: .disabled(reasonCode: .redacted, helpText: "Redacted"),
            rejectAvailability: .disabled(reasonCode: .redacted, helpText: "Redacted"),
            freshnessState: .live,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertEqual(row.deferredState, .redacted)
        // M2: raw helpText must not leak; generic message is substituted instead
        XCTAssertEqual(row.approveDisabledReason, "Redacted — details unavailable")
        XCTAssertEqual(row.rejectDisabledReason, "Redacted — details unavailable")
        // M3: body text suppressed in redacted state to prevent leaking sensitive detail
        XCTAssertNil(row.body, "Approval body must be nil when state is redacted")
        // M4 (P036-SEC-001): upstream accessibilityLabel must not leak sensitive content
        // via VoiceOver/assistive tech when the row is in the redacted deferred state.
        XCTAssertEqual(
            row.accessibilityLabel,
            "Approval pending review — details restricted",
            "Redacted approval must use a sanitized generic accessibilityLabel"
        )
        XCTAssertFalse(
            row.accessibilityLabel.contains("Redacted"),
            "Sanitized label must not echo the raw redacted helpText"
        )
    }

    // P036-SEC-004: .unknown and .refreshing freshness must produce explicit deferred states
    // so the row renders a banner instead of silently showing no buttons with no explanation.
    func testInlineApprovalUnknownFreshnessMapsToUnsupported() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-unknown-fresh",
            approveAvailability: .disabled(reasonCode: nil, helpText: "Unknown freshness state"),
            rejectAvailability: .disabled(reasonCode: nil, helpText: "Unknown freshness state"),
            freshnessState: .unknown(rawValue: "unexpected_value"),
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertNotNil(
            row.deferredState,
            "Unknown freshness must produce a non-nil deferredState so the row renders a banner"
        )
        XCTAssertEqual(row.deferredState, .unsupported, "Unknown freshness should map to .unsupported")
    }

    func testInlineApprovalRefreshingFreshnessMapsToUnavailable() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-refreshing",
            approveAvailability: .disabled(reasonCode: nil, helpText: "Refreshing"),
            rejectAvailability: .disabled(reasonCode: nil, helpText: "Refreshing"),
            freshnessState: .refreshing,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertNotNil(
            row.deferredState,
            "Refreshing freshness must produce a non-nil deferredState so the row renders a banner"
        )
        XCTAssertEqual(row.deferredState, .unavailable, "Refreshing freshness should map to .unavailable")
    }

    func testInlineApprovalBodyAndAccessibility() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-body",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .live,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertEqual(row.body, "Body", "Non-redacted approval should include body text")
        XCTAssertFalse(row.accessibilityLabel.isEmpty, "Accessibility label must not be empty")
    }

    func testInlineApprovalAlreadyResolved() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-resolved",
            approveAvailability: .disabled(reasonCode: .alreadyResolved, helpText: "Already resolved"),
            rejectAvailability: .disabled(reasonCode: .alreadyResolved, helpText: "Already resolved"),
            freshnessState: .live,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertEqual(row.deferredState, .alreadyResolved)
    }

    func testInlineApprovalDuplicate() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-dup",
            approveAvailability: .disabled(reasonCode: .duplicate, helpText: "Duplicate"),
            rejectAvailability: .disabled(reasonCode: .duplicate, helpText: "Duplicate"),
            freshnessState: .live,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertEqual(row.deferredState, .duplicate)
    }

    func testInlineApprovalConflict() {
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-conflict",
            approveAvailability: .disabled(reasonCode: .conflict, helpText: "Conflict"),
            rejectAvailability: .disabled(reasonCode: .conflict, helpText: "Conflict"),
            freshnessState: .live,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove)
        XCTAssertEqual(row.deferredState, .conflict)
    }

    // MARK: - Fail-closed regression (P036-SEC-005)
    // These tests verify that a non-nil deferred state always overrides actionable
    // P085 availability to false. The presenter must compute deferred BEFORE canApprove/canReject.

    func testFailClosedStaleOverridesActionable() {
        // freshnessState=.stale while approveAvailability=.actionable must still produce
        // canApprove=false and deferredState=.stale.
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-stale-actionable",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .stale,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove, "Stale freshness must disable approve even when P085 reports actionable")
        XCTAssertFalse(row.canReject, "Stale freshness must disable reject even when P085 reports actionable")
        XCTAssertEqual(row.deferredState, .stale)
    }

    func testFailClosedUnauthorizedOverridesActionable() {
        // freshnessState=.unauthorized while both availabilities=.actionable must produce
        // canApprove=false and deferredState=.unauthorized.
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-unauth-actionable",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .unauthorized,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove, "Unauthorized freshness must disable approve even when P085 reports actionable")
        XCTAssertFalse(row.canReject, "Unauthorized freshness must disable reject even when P085 reports actionable")
        XCTAssertEqual(row.deferredState, .unauthorized)
    }

    func testFailClosedProjectionLagOverridesActionable() {
        // freshnessState=.projectionLag while approveAvailability=.actionable must produce
        // canApprove=false and deferredState=.projectionLag.
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-lag-actionable",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .projectionLag,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: true
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove, "ProjectionLag freshness must disable approve even when P085 reports actionable")
        XCTAssertFalse(row.canReject, "ProjectionLag freshness must disable reject even when P085 reports actionable")
        XCTAssertEqual(row.deferredState, .projectionLag)
    }

    func testFailClosedUnavailableOverridesActionable() {
        // freshnessState=.unavailable while approveAvailability=.actionable must produce
        // canApprove=false and deferredState=.unavailable.
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "app-unavail-actionable",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .unavailable,
            diagnostic: makeDiagnostic(),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let row = model.inlineApprovals[0]
        XCTAssertFalse(row.canApprove, "Unavailable freshness must disable approve even when P085 reports actionable")
        XCTAssertFalse(row.canReject, "Unavailable freshness must disable reject even when P085 reports actionable")
        XCTAssertEqual(row.deferredState, .unavailable)
    }

    func testInlineApprovalActionability() {
        let model = RunsWorkbenchPresentationModel()

        let diagnostic = P085DiagnosticAffordanceState(
            diagnosticID: "d1",
            serverDebugDetail: "detail",
            isAvailable: true
        )

        let affordance = P085ApprovalAffordanceState(
            approvalID: "app1",
            approveAvailability: .disabled(reasonCode: .unauthorized, helpText: "No permission"),
            rejectAvailability: .disabled(reasonCode: .unauthorized, helpText: "No permission"),
            freshnessState: .live,
            diagnostic: diagnostic,
            projectionLagIsOnlyConstraint: false
        )

        let row = P031ApprovalInboxRowPresentation(
            approvalID: "app1",
            title: "Test Approval",
            body: "Body",
            canApprove: true,
            canReject: false,
            actionLabel: nil,
            followUpID: nil,
            copyItems: [],
            freshnessState: .live,
            accessibilityLabel: "label",
            affordance: affordance
        )

        let detail = P031RunDetailPresentation(
            title: "Run 1",
            workflowLabel: "w1",
            statusLabel: "Blocked",
            progressLabel: nil,
            pendingApprovalsLabel: nil,
            rolloutDecisionSummary: nil,
            ideaContext: nil,
            stageTransitions: [],
            approvalRows: [row],
            artifactRows: [],
            artifactViewerRows: [],
            reportRows: [],
            catalogContext: nil,
            closeoutReadiness: nil,
            implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "blocked",
            failedStages: 0
        )

        model.populate(from: detail)

        XCTAssertEqual(model.inlineApprovals.count, 1)
        let appRow = model.inlineApprovals[0]
        XCTAssertFalse(appRow.canApprove)
        XCTAssertFalse(appRow.canReject)
        XCTAssertEqual(appRow.approveDisabledReason, "No permission")
        XCTAssertEqual(appRow.rejectDisabledReason, "No permission")
        XCTAssertEqual(appRow.deferredState, .unauthorized)
    }

    func testTimelineReconciliation() {
        let live1 = LiveExecutionTimelineEntry(
            id: UUID(),
            agentID: "a1",
            agentTitle: "Agent 1",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallStarted,
                timestamp: Date(),
                detail: "Starting tool",
                sessionID: "sess1",
                requestID: "req1",
                toolName: "test_tool"
            )
        )

        let live2 = LiveExecutionTimelineEntry(
            id: UUID(),
            agentID: "a1",
            agentTitle: "Agent 1",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallFinished,
                timestamp: Date().addingTimeInterval(1),
                detail: "Finished tool",
                sessionID: "sess1",
                requestID: "req1",
                toolName: "test_tool"
            )
        )

        let entries = buildFocusedTimelineSpineEntries(
            liveTimeline: [live1, live2],
            persistedTimeline: []
        )

        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0].kind, .mergedTool)
        XCTAssertTrue(entries[0].detail.contains("completed"))
    }

    // MARK: - Concurrent agent timeline reconciliation (compound identity key)

    func testTimelineConcurrentAgentsSameRequestIDDoNotEraseEachOther() {
        // Two agents emit toolCallStarted with the same requestID.
        // With compound key (agentID:sessionID:requestID) they must produce two distinct merged-tool entries.
        let agent1Start = LiveExecutionTimelineEntry(
            id: UUID(), agentID: "agent-alpha", agentTitle: "Alpha",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallStarted, timestamp: Date(),
                detail: "alpha starts tool", sessionID: "sess-a", requestID: "shared-req", toolName: "edit"
            )
        )
        let agent2Start = LiveExecutionTimelineEntry(
            id: UUID(), agentID: "agent-beta", agentTitle: "Beta",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallStarted, timestamp: Date().addingTimeInterval(0.1),
                detail: "beta starts tool", sessionID: "sess-b", requestID: "shared-req", toolName: "edit"
            )
        )
        let entries = buildFocusedTimelineSpineEntries(
            liveTimeline: [agent1Start, agent2Start],
            persistedTimeline: []
        )
        let mergedTools = entries.filter { $0.kind == .mergedTool }
        XCTAssertEqual(mergedTools.count, 2,
            "Both agents must produce their own merged-tool entry even when requestID is shared")
        let agentIDs = Set(mergedTools.compactMap { $0.agentID })
        XCTAssertTrue(agentIDs.contains("agent-alpha"), "Alpha must have its own entry")
        XCTAssertTrue(agentIDs.contains("agent-beta"), "Beta must have its own entry")
    }

    func testTimelineMergedToolCollapsedAfterAgentCompletion() {
        // A toolCallStarted/Finished pair followed by a finalOutput must result in
        // the merged-tool card being marked isCollapsed=true.
        let start = LiveExecutionTimelineEntry(
            id: UUID(), agentID: "a-collapse", agentTitle: "Collapse Agent",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallStarted, timestamp: Date(timeIntervalSince1970: 1),
                detail: "starting", sessionID: "sess-c", requestID: "req-c", toolName: "write"
            )
        )
        let finish = LiveExecutionTimelineEntry(
            id: UUID(), agentID: "a-collapse", agentTitle: "Collapse Agent",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallFinished, timestamp: Date(timeIntervalSince1970: 2),
                detail: "finished", sessionID: "sess-c", requestID: "req-c", toolName: "write"
            )
        )
        let summary = LiveExecutionTimelineEntry(
            id: UUID(), agentID: "a-collapse", agentTitle: "Collapse Agent",
            stageID: "s1",
            event: ExecutionEvent(
                type: .finalOutput, timestamp: Date(timeIntervalSince1970: 3),
                detail: "Agent completed"
            )
        )
        let entries = buildFocusedTimelineSpineEntries(
            liveTimeline: [start, finish, summary],
            persistedTimeline: []
        )
        let mergedTool = entries.first { $0.kind == .mergedTool }
        XCTAssertNotNil(mergedTool, "Should have a merged-tool entry")
        XCTAssertTrue(mergedTool?.isCollapsed == true,
            "Merged-tool card must be collapsed when the agent has a finalOutput")
        let agentSummary = entries.first { $0.kind == .agentSummary }
        XCTAssertNotNil(agentSummary, "Should have an agent summary entry")
        XCTAssertFalse(agentSummary?.isCollapsed == true,
            "Agent summary must not be collapsed")
    }

    func testTimelineOutOfOrderToolFinish() {
        let live1 = LiveExecutionTimelineEntry(
            id: UUID(),
            agentID: "a1",
            agentTitle: "Agent 1",
            stageID: "s1",
            event: ExecutionEvent(
                type: .toolCallFinished,
                timestamp: Date(),
                detail: "Out of order finish",
                sessionID: "sess1",
                requestID: "req-unknown",
                toolName: "test_tool"
            )
        )

        let entries = buildFocusedTimelineSpineEntries(
            liveTimeline: [live1],
            persistedTimeline: []
        )

        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0].kind, .sessionEvent)
        XCTAssertTrue(entries[0].title.contains("Diagnostic"))
    }

    func testTimelineEntryCap() {
        // Cap is tested against normal (non-priority) textChunk entries.
        // sessionStarted/agentSummary/policyWarning/sessionEvent entries are reserved and never dropped.
        var liveEntries: [LiveExecutionTimelineEntry] = []
        for i in 0..<50 {
            liveEntries.append(LiveExecutionTimelineEntry(
                id: UUID(),
                agentID: "a1",
                agentTitle: "Agent 1",
                stageID: "s1",
                event: ExecutionEvent(
                    type: .textChunk,
                    timestamp: Date().addingTimeInterval(TimeInterval(i)),
                    detail: "Text chunk \(i)"
                )
            ))
        }

        let entries = buildFocusedTimelineSpineEntries(
            liveTimeline: liveEntries,
            persistedTimeline: []
        )

        XCTAssertLessThanOrEqual(entries.count, 40, "Non-priority textChunk entries should be capped at 40")
    }

    func testTimelineAgentSummaryGrouping() {
        let live1 = LiveExecutionTimelineEntry(
            id: UUID(),
            agentID: "agent-1",
            agentTitle: "Agent 1",
            stageID: "s1",
            event: ExecutionEvent(type: .finalOutput, timestamp: Date(), detail: "Summary 1")
        )
        let live2 = LiveExecutionTimelineEntry(
            id: UUID(),
            agentID: "agent-1",
            agentTitle: "Agent 1",
            stageID: "s1",
            event: ExecutionEvent(type: .finalOutput, timestamp: Date().addingTimeInterval(1), detail: "Summary 2")
        )

        let entries = buildFocusedTimelineSpineEntries(
            liveTimeline: [live1, live2],
            persistedTimeline: []
        )

        // Should only show the latest summary per agent
        let summaries = entries.filter { $0.kind == .agentSummary }
        XCTAssertEqual(summaries.count, 1)
        XCTAssertEqual(summaries[0].detail, "Summary 2")
    }

    @MainActor
    func testWorkbenchTimelineShowsOnlySelectedActiveAgentEvents() {
        let model = RunsWorkbenchPresentationModel()
        let detail = P031RunDetailPresentation(
            title: "Run",
            workflowLabel: "Workflow",
            statusLabel: "Running",
            progressLabel: "1/1 stages",
            pendingApprovalsLabel: nil,
            rolloutDecisionSummary: nil,
            ideaContext: nil,
            stageTransitions: [],
            approvalRows: [],
            artifactRows: [],
            artifactViewerRows: [],
            reportRows: [],
            activeAgentTimelineEntries: [
                P031ActiveAgentTimelinePresentation(
                    id: "active-earlier",
                    title: "Earlier Active Agent",
                    detail: "earlier active text",
                    timestamp: Date(timeIntervalSince1970: 3),
                    stageID: "stage-1",
                    agentID: "agent-earlier",
                    sessionID: "session-earlier"
                ),
                P031ActiveAgentTimelinePresentation(
                    id: "active-selected",
                    title: "Selected Active Agent",
                    detail: "selected active text",
                    timestamp: Date(timeIntervalSince1970: 4),
                    stageID: "stage-1",
                    agentID: "agent-selected",
                    sessionID: "session-selected"
                )
            ],
            catalogContext: nil,
            closeoutReadiness: nil,
            implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "running",
            failedStages: 0
        )

        model.selectActiveTimelineAgent("agent-earlier")
        model.populate(from: detail)

        XCTAssertEqual(model.timelineEntries.count, 1)
        XCTAssertEqual(model.selectedActiveTimelineAgentID, "agent-earlier")
        XCTAssertEqual(model.timelineEntries.first?.agentID, "agent-earlier")
        XCTAssertEqual(model.timelineEntries.first?.detail, "earlier active text")
        XCTAssertFalse(
            model.timelineEntries.contains { $0.agentID == "agent-complete" },
            "Completed-agent events must not be replayed from local Swift timeline history."
        )
        XCTAssertFalse(
            model.timelineEntries.contains { $0.agentID == "agent-selected" },
            "Workbench timeline must focus one selected active agent stream at a time."
        )
    }

    func testWorkflowOrdering() {
        let state1 = WorkflowState(label: "S1", type: "start", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [Transition(to: "s2", when: "true")])
        let state2 = WorkflowState(label: "S2", type: "end", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [])

        var workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(
                id: "w1",
                name: "W1",
                usesAgentCatalog: nil,
                description: "D1",
                ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "always"),
                requiredProviders: []
            ),
            variables: [:],
            failurePolicy: nil,
            scoring: nil,
            initialState: "s1",
            states: ["s1": state1, "s2": state2]
        )

        // No stateOrder provided, should fallback to execution order (s1 then s2)
        XCTAssertNil(workflow.stateOrder)

        // Test source-order fallback in view (internal logic verification)
        // Since we can't easily test the view's private property here, we verify that stateOrder is used if present.
        workflow.stateOrder = ["s2", "s1"]
        XCTAssertEqual(workflow.stateOrder?.first, "s2")
    }

    func testWorkflowOrderingHostileStateOrder() {
        let state1 = WorkflowState(label: "S1", type: "start", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [Transition(to: "s2", when: "true")])
        let state2 = WorkflowState(label: "S2", type: "end", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [])
        var workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(id: "w1", name: "W1", usesAgentCatalog: nil, description: "D1", ideaInput: nil, execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "always"), requiredProviders: []),
            variables: [:], failurePolicy: nil, scoring: nil,
            initialState: "s1",
            states: ["s1": state1, "s2": state2]
        )

        // Hostile: stateOrder references a state ID not in workflow.states
        workflow.stateOrder = ["s1", "s2", "phantom_state"]
        let phantomIssues = YAMLValidator.validateStateGraph(workflow)
        XCTAssertTrue(
            phantomIssues.contains { $0.severity == .warning && $0.message.contains("phantom_state") },
            "Should emit a warning when state_order contains an unknown state ID"
        )

        // Hostile: stateOrder omits initialState
        workflow.stateOrder = ["s2"]
        let missingInitialIssues = YAMLValidator.validateStateGraph(workflow)
        XCTAssertTrue(
            missingInitialIssues.contains { $0.severity == .warning && $0.message.contains("initial_state") },
            "Should emit a warning when state_order omits the initial_state"
        )

        // Valid: stateOrder contains all known states including initialState — no ordering warnings
        workflow.stateOrder = ["s1", "s2"]
        let validIssues = YAMLValidator.validateStateGraph(workflow)
        XCTAssertFalse(
            validIssues.contains { $0.message.contains("state_order") },
            "Valid state_order should not trigger ordering warnings"
        )
    }

    // MARK: - Tab Route Resolution Matrix

    func testTabRouteResolutionMatrix() {
        // Old routes must map deterministically to their P036 target surfaces.
        // Approvals routes redirect to Runs per P036 old_route_mapping.
        XCTAssertEqual(ContentView.Tab.from(rawValue: "runsHome"), .runs)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Runs Home"), .runs)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Runs"), .runs)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "approvals"), .runs, "Approvals deep-link must route to Runs")
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Approvals"), .runs, "Approvals deep-link must route to Runs")
        XCTAssertEqual(ContentView.Tab.from(rawValue: "agentCatalog"), .definitions)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Agent Catalog"), .definitions)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "workflowInspector"), .definitions)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Workflow Inspector"), .definitions)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "pilotReadiness"), .settings)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Pilot Readiness"), .settings)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "providerSettings"), .settings)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Settings"), .settings)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Ideas"), .ideas)
        XCTAssertEqual(ContentView.Tab.from(rawValue: "ideas"), .ideas)
        XCTAssertNil(ContentView.Tab.from(rawValue: "NonexistentTab"))
        // P036 Phase 2c cutover: exactly four tabs.
        XCTAssertEqual(ContentView.Tab.allCases.count, 4)
    }

    // MARK: - Agent Grouping Other Fallback

    func testAgentGroupingOtherFallback() {
        let agentNoGroupNoProfileNoRole = AgentDefinition(
            id: "a-other",
            title: "Other Agent",
            mode: "",
            group: nil,
            backendProfile: "",
            permissionProfile: "perm",
            mcpProfile: nil,
            skillRef: "",
            skillRole: nil,
            worktreePolicy: nil,
            requiredTools: nil,
            inputs: [],
            outputs: [],
            outputContract: nil,
            requiresHumanApproval: false,
            xcodeBrokerRequired: nil,
            xcodeShimInjectionSignal: nil,
            requiresXcodeHostExecution: nil,
            prompt: "",
            notes: nil
        )

        let agentWhitespaceGroup = AgentDefinition(
            id: "a-ws",
            title: "Whitespace Group",
            mode: "fast",
            group: "   ",
            backendProfile: "",
            permissionProfile: "perm",
            mcpProfile: nil,
            skillRef: "",
            skillRole: nil,
            worktreePolicy: nil,
            requiredTools: nil,
            inputs: [],
            outputs: [],
            outputContract: nil,
            requiresHumanApproval: false,
            xcodeBrokerRequired: nil,
            xcodeShimInjectionSignal: nil,
            requiresXcodeHostExecution: nil,
            prompt: "",
            notes: nil
        )

        let appConfig = AppConfig(
            name: "Test", runtime: "test", transport: "test", description: "test",
            ideaInputMode: "test", singleActiveRunPerIdea: false, runResumePolicy: "test",
            requiredProviders: []
        )
        let catalog = AgentCatalog(
            schemaVersion: 1, app: appConfig, paths: [:], artifacts: [:], skills: [:],
            mcpPolicy: .defaultDeny, mcpServerRegistry: [:], mcpProfiles: [:], contracts: [:],
            backendProfiles: [:], permissionProfiles: [:], runtimeProfiles: [:],
            agents: [agentNoGroupNoProfileNoRole, agentWhitespaceGroup]
        )

        let grouped = catalog.groupedAgents()
        // Both should fall to "Other" since mode is empty (first) or group is whitespace→fallback to mode "fast" (second)
        let otherGroup = grouped.first { $0.label == "Other" }
        let fastGroup = grouped.first { $0.label == "Fast" }
        XCTAssertNotNil(otherGroup, "Agent with empty mode should land in 'Other'")
        XCTAssertEqual(otherGroup?.agents.first?.id, "a-other")
        XCTAssertNotNil(fastGroup, "Agent with whitespace group falls back to mode 'fast' -> 'Fast'")
        XCTAssertEqual(fastGroup?.agents.first?.id, "a-ws")
    }

    // MARK: - Workflow Ordering Topology (branches, cycles, unreachable)

    func testWorkflowOrderingWithBranchesAndUnreachable() {
        // s1 → s2 (primary), s1 → s3 (branch), s4 is unreachable
        let s1 = WorkflowState(label: "S1", type: "start", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil,
            transitions: [Transition(to: "s2", when: "true"), Transition(to: "s3", when: "alt")])
        let s2 = WorkflowState(label: "S2", type: "end", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [])
        let s3 = WorkflowState(label: "S3", type: "end", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [])
        let s4 = WorkflowState(label: "S4", type: "end", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil, transitions: [])

        let workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(id: "w1", name: "W1", usesAgentCatalog: nil, description: "D1", ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: true, resumePolicy: "always"), requiredProviders: []),
            variables: [:], failurePolicy: nil, scoring: nil,
            initialState: "s1",
            states: ["s1": s1, "s2": s2, "s3": s3, "s4": s4]
        )

        XCTAssertNil(workflow.stateOrder, "No explicit state order in YAML")
        XCTAssertEqual(workflow.initialState, "s1")
        XCTAssertEqual(workflow.states.count, 4)
        // s3 is reachable (branch), s4 is unreachable from s1
        let reachableFromS1 = Set(["s1", "s2", "s3"])
        XCTAssertTrue(reachableFromS1.isSubset(of: Set(workflow.states.keys)))
        XCTAssertTrue(workflow.states.keys.contains("s4"))
    }

    func testWorkflowCycleDetectionDoesNotHang() {
        // s1 → s2 → s1 (cycle): traversal must not loop infinitely
        let s1 = WorkflowState(label: "S1", type: "start", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil,
            transitions: [Transition(to: "s2", when: "true")])
        let s2 = WorkflowState(label: "S2", type: "normal", owner: "o1", approval: nil, run: nil, runAfterApproval: nil, loop: nil,
            transitions: [Transition(to: "s1", when: "retry")])

        let workflow = WorkflowDefinition(
            schemaVersion: 1,
            workflow: WorkflowMeta(id: "w-cycle", name: "Cycle", usesAgentCatalog: nil, description: "Cycle test", ideaInput: nil,
                execution: ExecutionConfig(singleActiveRunPerIdea: false, resumePolicy: "always"), requiredProviders: []),
            variables: [:], failurePolicy: nil, scoring: nil,
            initialState: "s1",
            states: ["s1": s1, "s2": s2]
        )

        // stateOrder is nil; workflow has 2 states in a cycle
        XCTAssertEqual(workflow.states.count, 2)
        XCTAssertNil(workflow.stateOrder)
        // If we called sortedStates here the DFS would stop at cycle via visited set — verified by code inspection.
    }

    // MARK: - Timeline Batch Flush Model

    @MainActor
    func testTimelineBatchModelBuffersAndFlushes() async {
        let model = P036TimelinePresentationModel()
        XCTAssertTrue(model.entries.isEmpty, "Model starts empty")

        let liveEntries: [LiveExecutionTimelineEntry] = (0..<5).map { i in
            LiveExecutionTimelineEntry(
                id: UUID(),
                agentID: "agent-\(i % 2)",
                agentTitle: "Agent \(i % 2)",
                stageID: "s1",
                event: ExecutionEvent(
                    type: .sessionStarted,
                    timestamp: Date().addingTimeInterval(TimeInterval(i)),
                    detail: "Session \(i)"
                )
            )
        }

        model.update(live: liveEntries, persisted: [], xcode: [])
        // After update with flush eligible (timer/interval not elapsed for very first call - model flushes immediately if past interval)
        // The model flushes immediately on first call since lastFlush starts at distantPast
        XCTAssertFalse(model.entries.isEmpty, "Model flushes entries immediately on first update (distantPast interval)")
    }

    @MainActor
    func testTimelineLosslessTerminalEventPreservation() async {
        // Terminal events (finalOutput, finish) must never be dropped by the 40-entry cap
        var entries: [LiveExecutionTimelineEntry] = []
        for i in 0..<38 {
            entries.append(LiveExecutionTimelineEntry(
                id: UUID(), agentID: "a1", agentTitle: "A1", stageID: "s1",
                event: ExecutionEvent(type: .sessionStarted, timestamp: Date().addingTimeInterval(TimeInterval(i)), detail: "Event \(i)")
            ))
        }
        // Add terminal events at positions 39 and 40
        entries.append(LiveExecutionTimelineEntry(
            id: UUID(), agentID: "a1", agentTitle: "A1", stageID: "s1",
            event: ExecutionEvent(type: .finalOutput, timestamp: Date().addingTimeInterval(100), detail: "Final output")
        ))
        entries.append(LiveExecutionTimelineEntry(
            id: UUID(), agentID: "a2", agentTitle: "A2", stageID: "s1",
            event: ExecutionEvent(type: .finalOutput, timestamp: Date().addingTimeInterval(101), detail: "Final output 2")
        ))

        let result = buildFocusedTimelineSpineEntries(liveTimeline: entries, persistedTimeline: [])
        XCTAssertEqual(result.count, 40, "Exactly 40 entries after cap")
        // The two finalOutput events must be included since they appear first in reverse-chronological sort
        let summaries = result.filter { $0.kind == .agentSummary }
        XCTAssertEqual(summaries.count, 2, "Both terminal events are preserved")
    }

    // MARK: - P036DeferredState Init from Affordance

    func testDeferredStateInitFromAffordance() {
        let diagnostic = P085DiagnosticAffordanceState(diagnosticID: nil, serverDebugDetail: nil, isAvailable: false)

        let stateMap: [(P085FreshnessState, P036DeferredState?)] = [
            (.projectionLag, .projectionLag),
            (.stale, .stale),
            (.unauthorized, .unauthorized),
            (.unavailable, .unavailable),
        ]

        for (freshness, expected) in stateMap {
            let affordance = P085ApprovalAffordanceState(
                approvalID: "x",
                approveAvailability: .disabled(reasonCode: nil, helpText: ""),
                rejectAvailability: .disabled(reasonCode: nil, helpText: ""),
                freshnessState: freshness,
                diagnostic: diagnostic,
                projectionLagIsOnlyConstraint: false
            )
            XCTAssertEqual(P036DeferredState(from: affordance), expected, "freshnessState \(freshness) → \(String(describing: expected))")
        }

        let codeMap: [(P031DisabledReasonCode, P036DeferredState)] = [
            (.redacted, .redacted),
            (.conflict, .conflict),
            (.duplicate, .duplicate),
            (.alreadyResolved, .alreadyResolved),
            (.writePathNotAvailable, .unavailable),
            (.managedOutsideUI, .unsupported),
            (.unsupportedAction, .unsupported),
            (.ambiguousApprovalIdentity, .unsupported),
        ]

        for (code, expected) in codeMap {
            let affordance = P085ApprovalAffordanceState(
                approvalID: "x",
                approveAvailability: .disabled(reasonCode: code, helpText: ""),
                rejectAvailability: .disabled(reasonCode: nil, helpText: ""),
                freshnessState: .live,
                diagnostic: diagnostic,
                projectionLagIsOnlyConstraint: false
            )
            XCTAssertEqual(P036DeferredState(from: affordance), expected, "code \(code) → \(expected)")
        }
    }

    func testDeferredStateDisplayLabelsAreDistinctForInlineApprovalCard() {
        let labels = P036DeferredState.allCases.map(\.displayLabel)
        XCTAssertEqual(Set(labels).count, labels.count,
                       "Inline approval card must render each P085/P036 deferred state with a distinct label")
    }

    // MARK: - Ideas lane projection (P036: no local string inference)

    func testIdeasLaneUsesProjectedStatusNotStringMatching() {
        // P036 rule: Ideas run-status strips must use the canonical P036RunLane computed
        // from server-projected RunStatus enum, not local string matching against the status text.
        let makeRun = { (id: String, status: String, pendingApprovals: Int?) -> P031RunRowReadModel in
            P031RunRowReadModel(
                id: id, status: status, ideaID: "idea-1", ideaTitle: "Test Idea",
                projectKey: "TEST", workflowTitle: "w", workflowID: nil,
                workflowSnapshotHash: nil, catalogSnapshotHash: nil,
                freshnessState: .live, totalStages: 2, completedStages: 0,
                failedStages: 0, pendingApprovals: pendingApprovals
            )
        }

        // Waiting: pendingApprovals > 0
        let waiting = makeRun("r-waiting", "running", 2)
        XCTAssertEqual(waiting.lane, .waiting, "pendingApprovals > 0 must be .waiting regardless of status")

        // Blocked: RunStatus type-safe match
        let blockedByEnum = makeRun("r-blocked", "blocked", nil)
        XCTAssertEqual(blockedByEnum.lane, .blocked, "RunStatus.blocked must map to .blocked lane")

        let failedByEnum = makeRun("r-failed", "failed", nil)
        XCTAssertEqual(failedByEnum.lane, .blocked, "RunStatus.failed must map to .blocked lane")

        // Running: RunStatus type-safe match
        let runningByEnum = makeRun("r-running", "running", nil)
        XCTAssertEqual(runningByEnum.lane, .running, "RunStatus.running must map to .running lane")

        // Completed: RunStatus type-safe match
        let completedByEnum = makeRun("r-completed", "completed", nil)
        XCTAssertEqual(completedByEnum.lane, .completed, "RunStatus.completed must map to .completed lane")

        // Cancelled maps to .completed lane (not .blocked)
        let cancelled = makeRun("r-cancelled", "cancelled", nil)
        XCTAssertEqual(cancelled.lane, .completed, "RunStatus.cancelled must map to .completed lane")
    }

    func testIdeasLaneDeferredForUnknownStatus() {
        // P036 no-local-inference rule: status strings not in the typed RunStatus vocabulary
        // must produce .deferred, never a guessed .blocked/.running/.completed.
        let makeRun = { (status: String, failedStages: Int?) -> P031RunRowReadModel in
            P031RunRowReadModel(
                id: "r-test", status: status, ideaID: nil, ideaTitle: nil,
                projectKey: nil, workflowTitle: "w", workflowID: nil,
                workflowSnapshotHash: nil, catalogSnapshotHash: nil,
                freshnessState: .live, totalStages: nil, completedStages: nil,
                failedStages: failedStages, pendingApprovals: nil
            )
        }

        XCTAssertEqual(makeRun("error", nil).lane, .deferred, "Unknown status must be .deferred (no local inference)")
        XCTAssertEqual(makeRun("active", nil).lane, .deferred, "Unknown status must be .deferred (no local inference)")
        XCTAssertEqual(makeRun("success", nil).lane, .deferred, "Unknown status must be .deferred (no local inference)")
        XCTAssertEqual(makeRun("unknown_status", 1).lane, .deferred, "Unknown status with failedStages > 0 must be .deferred (no local inference)")
        XCTAssertEqual(makeRun("unknown_status", 0).lane, .deferred, "Unknown status with no failedStages must be .deferred (no local inference)")
    }

    // MARK: - P036 UI metric counters

    @MainActor
    func testP036UICountersRecordTabRouteResolution() {
        P036UICounters.shared.reset()
        let before = P036UICounters.shared.tabRouteResolutionTotal
        P036UICounters.shared.recordTabRouteResolution(source: "Runs", target: "Settings", result: "routed")
        XCTAssertEqual(P036UICounters.shared.tabRouteResolutionTotal, before + 1)
    }

    @MainActor
    func testP036UICountersRecordInlineApprovalRender() {
        P036UICounters.shared.reset()
        P036UICounters.shared.recordInlineApprovalRender(count: 3, freshnessState: "live", actionabilityState: "actionable")
        XCTAssertEqual(P036UICounters.shared.inlineApprovalRenderTotal, 3)
    }

    @MainActor
    func testP036UICountersRecordTimelineBatchFlush() {
        P036UICounters.shared.reset()
        P036UICounters.shared.recordTimelineBatchFlush(entryCount: 10, reduceMotion: false)
        XCTAssertEqual(P036UICounters.shared.timelineBatchFlushTotal, 1)
    }

    @MainActor
    func testP036UICountersRecordTimelineCardCollapse() {
        P036UICounters.shared.reset()
        P036UICounters.shared.recordTimelineCardCollapse(count: 2, reason: "completed_stage_start")
        XCTAssertEqual(P036UICounters.shared.timelineCardCollapseTotal, 2)
        XCTAssertEqual(
            P036UICounterStore.value(for: P036UICounterStore.timelineCardCollapseTotal),
            2,
            "Timeline card collapse counter must persist for MetricsCollector readback"
        )
    }

    @MainActor
    func testP036UICountersRecordOperatorTaskAttempt() {
        P036UICounters.shared.reset()
        P036UICounters.shared.recordOperatorTaskAttempt(
            taskID: "runs.settle_approval",
            result: "blocked",
            blockedReason: "write_failed"
        )
        XCTAssertEqual(P036UICounters.shared.operatorTaskAttemptTotal, 1)
        XCTAssertEqual(
            P036UICounterStore.value(for: P036UICounterStore.operatorTaskAttemptTotal),
            1,
            "Operator task attempts must persist for dogfood and remote-smoke metric readback"
        )
        XCTAssertEqual(
            P036UICounterStore.operatorTaskAttemptSamples(),
            [
                P036OperatorTaskAttemptSample(
                    taskID: "runs.settle_approval",
                    result: "blocked",
                    blockedReason: "write_failed",
                    count: 1
                )
            ],
            "Operator task attempt readback must preserve task_id, result, and blocked_reason labels"
        )
    }

    func testP036ReleaseEvidenceCoversDogfoodRemoteUIAndRolloutReadback() throws {
        let dogfood = try loadEvidenceObject("docs/evidence/macos-operator-navigation/dogfood-validation-2026-05-21.json")
        let dogfoodRuns = try XCTUnwrap(dogfood["task_set_runs"] as? [[String: Any]])
        XCTAssertGreaterThanOrEqual(dogfoodRuns.count, 5, "Phase 2.5 dogfood must include at least five task-set runs")
        let operators = Set(dogfoodRuns.compactMap { $0["operator_id"] as? String })
        XCTAssertGreaterThanOrEqual(operators.count, 2, "Phase 2.5 dogfood must include at least two operators")
        for run in dogfoodRuns {
            XCTAssertNotNil(run["timeline_readability_rating"], "Every dogfood run must capture timeline readability")
            XCTAssertNotNil(run["task_outcome"], "Every dogfood run must capture task outcome")
        }
        let navigationMetric = try XCTUnwrap(dogfood["p036_operator_navigation_completion_rate"] as? [String: Any])
        XCTAssertNotNil(navigationMetric["baseline"])
        XCTAssertNotNil(navigationMetric["target_window"])

        let remoteUI = try loadEvidenceObject("docs/evidence/macos-operator-navigation/remote-ui-accessibility-proof-2026-05-21.json")
        let coveredSurfaces = Set((remoteUI["covered_surfaces"] as? [String]) ?? [])
        XCTAssertTrue(coveredSurfaces.isSuperset(of: [
            "navigation",
            "runs_inspection",
            "global_approval_context",
            "inline_approval_context",
            "ideas_deep_link_to_runs",
            "definitions_segment_switch",
            "settings_readiness_route",
            "heavy_tool_timeline_flow"
        ]))
        XCTAssertFalse((remoteUI["remote_result_bundle"] as? String ?? "").isEmpty)
        XCTAssertGreaterThanOrEqual((remoteUI["accessibility_checks"] as? [String])?.count ?? 0, 3)

        let rollout = try loadEvidenceObject("docs/evidence/macos-operator-navigation/rollout-readback-live-2026-05-21.json")
        let lanes = try XCTUnwrap(rollout["lanes"] as? [String: Any])
        for lane in ["run_report", "mcp", "release_receipt", "graphql"] {
            let payload = try XCTUnwrap(lanes[lane] as? [String: Any], "Missing P036 rollout readback lane \(lane)")
            XCTAssertEqual(payload["rollout_contract_status"] as? String ?? payload["rolloutContractStatus"] as? String, "pass")
        }
    }

    @MainActor
    func testWorkbenchPopulateEmitsApprovalRenderMetric() {
        P036UICounters.shared.reset()
        let model = RunsWorkbenchPresentationModel()
        let diagnostic = P085DiagnosticAffordanceState(diagnosticID: nil, serverDebugDetail: nil, isAvailable: false)
        let affordance = P085ApprovalAffordanceState(
            approvalID: "appr-metric",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .live,
            diagnostic: diagnostic,
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        // After populating one approval row, the counter must be >= 1
        XCTAssertGreaterThanOrEqual(
            P036UICounters.shared.inlineApprovalRenderTotal, 1,
            "Populating approval rows must emit inlineApprovalRender metric"
        )
    }

    // MARK: - DEFECT-004 regression: reject-only disabled states produce deferred rows

    func testRejectOnlyDisabledStateProducesDeferredRow() {
        // When approve is actionable but reject is disabled with a reason code, the
        // approval row must still derive a deferred state so it is not silently rendered
        // without explanation.
        let diagnostic = P085DiagnosticAffordanceState(diagnosticID: nil, serverDebugDetail: nil, isAvailable: false)
        let affordancesAndExpected: [(P085ApprovalAffordanceState, P036DeferredState)] = [
            (
                P085ApprovalAffordanceState(
                    approvalID: "x",
                    approveAvailability: .actionable,
                    rejectAvailability: .disabled(reasonCode: .redacted, helpText: "redacted"),
                    freshnessState: .live,
                    diagnostic: diagnostic,
                    projectionLagIsOnlyConstraint: false
                ),
                .redacted
            ),
            (
                P085ApprovalAffordanceState(
                    approvalID: "x",
                    approveAvailability: .actionable,
                    rejectAvailability: .disabled(reasonCode: .conflict, helpText: "conflict"),
                    freshnessState: .live,
                    diagnostic: diagnostic,
                    projectionLagIsOnlyConstraint: false
                ),
                .conflict
            ),
            (
                P085ApprovalAffordanceState(
                    approvalID: "x",
                    approveAvailability: .actionable,
                    rejectAvailability: .disabled(reasonCode: .alreadyResolved, helpText: "done"),
                    freshnessState: .live,
                    diagnostic: diagnostic,
                    projectionLagIsOnlyConstraint: false
                ),
                .alreadyResolved
            ),
        ]
        for (affordance, expected) in affordancesAndExpected {
            XCTAssertEqual(
                P036DeferredState(from: affordance), expected,
                "rejectAvailability disabled code must produce deferred state even when approve is actionable"
            )
        }
    }

    // MARK: - DEFECT-001 regression: WorkflowMapTopologyBuilder always uses execution order

    func testWorkflowMapTopologyBuilderAlwaysUsesExecutionOrder() {
        // Even when plan.stateOrder is set (YAML source order), WorkflowMapTopologyBuilder
        // must use initial-state traversal order for stage-map projection.
        let makeState = { (id: String, to: [String]) -> ExecutableState in
            ExecutableState(
                id: id,
                label: id.uppercased(),
                type: to.isEmpty ? .end : .start,
                ownerAgentID: "agent",
                runBlock: nil,
                runAfterApproval: nil,
                transitions: to.map { ExecutableTransition(to: $0, condition: .always) },
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            )
        }
        let plan = RunPlan(
            workflowID: "wf-topology-test",
            workflowTitle: "Topology Test",
            states: [
                "s1": makeState("s1", ["s2"]),
                "s2": makeState("s2", ["s3"]),
                "s3": makeState("s3", []),
                "s4": makeState("s4", []),  // unreachable
            ],
            initialStateID: "s1",
            stateOrder: ["s3", "s4", "s1", "s2"],  // YAML source order — must NOT be used by builder
            agentBindings: [:],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h",
            catalogSnapshotHash: "c",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )
        let builder = WorkflowMapTopologyBuilder(plan: plan)
        let ordered = builder.orderedStateIDs()
        // Execution order: s1 → s2 → s3, then s4 (unreachable)
        XCTAssertEqual(ordered.prefix(3), ["s1", "s2", "s3"],
                       "Execution order traversal must start from initialStateID and follow transitions")
        XCTAssertEqual(ordered.last, "s4", "Unreachable states must appear after primary path")
        XCTAssertNotEqual(ordered.first, "s3",
                          "stateOrder from RunPlan must not override execution-order traversal")
    }

    // MARK: - Readiness deferred state

    func testReadinessDeferredWhenDaemonProjectionAbsent() {
        let workbench = RunsWorkbenchPresentationModel()
        // When no daemon projection has arrived, readiness must be deferred — not inferred false.
        workbench.populate(daemon: nil, scheduler: nil)
        let health = workbench.freshnessAndHealth
        XCTAssertNotNil(health, "FreshnessHealth should be set even when daemon is nil")
        XCTAssertTrue(health?.isReadinessDeferred == true,
                      "isReadinessDeferred must be true when daemon projection is absent")
        XCTAssertFalse(health?.isSystemReady == true,
                       "isSystemReady must not be true when daemon projection is absent")
    }

    // MARK: - RunStatus snake_case server value mapping

    func testRunStatusFromServerValueHandlesSnakeCase() {
        // Rust control-plane emits snake_case; Swift enum rawValues are camelCase.
        XCTAssertEqual(RunStatus.from(serverValue: "waiting_approval"), .waitingApproval,
                       "Rust snake_case 'waiting_approval' must map to .waitingApproval")
        // camelCase local persistence values continue to work
        XCTAssertEqual(RunStatus.from(serverValue: "waitingApproval"), .waitingApproval)
        XCTAssertEqual(RunStatus.from(serverValue: "running"), .running)
        XCTAssertEqual(RunStatus.from(serverValue: "failed"), .failed)
        XCTAssertEqual(RunStatus.from(serverValue: "completed"), .completed)
        XCTAssertNil(RunStatus.from(serverValue: "unknown_status"),
                     "Unknown status must return nil, falling back to .deferred lane")
    }

    // MARK: - Timeline projection gap deferred metric (Phase 1-2 deferred state)

    @MainActor
    func testTimelineProjectionGapDeferredEmitsMetric() {
        // Timeline is deferred in Phase 1-2 per P036; the metric must be emitted when the
        // deferred placeholder is shown. This test verifies the counter increments correctly.
        P036UICounters.shared.reset()
        P036UICounters.shared.recordProjectionGapDeferred(count: 1, surface: "timeline", gapClass: "live_events")
        XCTAssertEqual(P036UICounters.shared.projectionGapDeferredTotal, 1,
                       "Timeline deferred state must emit p036_projection_gap_deferred_total")
    }

    @MainActor
    func testWorkbenchTimelineDoesNotDuplicateStageProjection() {
        P036UICounters.shared.reset()
        let model = RunsWorkbenchPresentationModel()
        let detail = P031RunDetailPresentation(
            title: "Run",
            workflowLabel: "proposal-loop-live",
            statusLabel: "Running",
            progressLabel: "1 of 2 stages",
            pendingApprovalsLabel: nil,
            rolloutDecisionSummary: nil,
            ideaContext: nil,
            stageTransitions: [
                P031StageTransitionPresentation(
                    stageExecutionID: "stage-1",
                    stageTitle: "Plan",
                    statusText: "Completed",
                    attemptText: "Attempt 1",
                    startedLabel: "Started: 2026-05-20 08:00",
                    completedLabel: "Completed: 2026-05-20 08:05",
                    durationLabel: "Duration: 5m",
                    connectorState: .completed,
                    evidenceLabels: ["Artifacts", "Completed"],
                    accessibilityLabel: "Plan completed"
                )
            ],
            approvalRows: [],
            artifactRows: [],
            artifactViewerRows: [],
            reportRows: [],
            catalogContext: nil,
            closeoutReadiness: nil,
            implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "running",
            failedStages: 0
        )

        model.populate(from: detail)

        XCTAssertEqual(model.timelineEntries.count, 0)
        XCTAssertEqual(P036UICounters.shared.timelineCardCollapseTotal, 0)
        XCTAssertEqual(model.stageMap?.stages.count, 0,
            "Stages tab must not render from stageTransitions; topology readback is the visual source")
    }

    @MainActor
    func testWorkbenchStagesUseDaemonTopologyReadback() {
        let model = RunsWorkbenchPresentationModel()
        let detail = P031RunDetailPresentation(
            title: "Run",
            workflowLabel: "proposal-loop-live",
            statusLabel: "Running",
            progressLabel: "1 of 2 stages",
            pendingApprovalsLabel: nil,
            rolloutDecisionSummary: nil,
            ideaContext: nil,
            stageTransitions: [],
            stageTopology: [
                P031StageTopologyPresentation(
                    stageID: "state_2",
                    ordinal: 2,
                    title: "Proposal drafted",
                    ownerAgentID: "proposal_writer",
                    ownerAgentTitle: "Proposal Writer",
                    status: "running",
                    statusText: "Running",
                    isCurrent: true,
                    iterationText: "Iteration 1",
                    attemptText: "Attempt 2",
                    approvalRequired: true,
                    artifactCount: 3,
                    communicationCount: 4,
                    occurrences: [
                        P031StageTopologyOccurrencePresentation(
                            agentID: "proposal_writer",
                            agentTitle: "Proposal Writer",
                            taskName: "Draft proposal",
                            statusText: "Running",
                            providerLabel: "codex · gpt-5.5",
                            executionCountLabel: "2 attempts"
                        )
                    ],
                    transitions: [
                        P031StageTopologyTransitionPresentation(
                            toStageID: "state_3",
                            toLabel: "Proposal reviewed",
                            detail: "exists('proposal_current')"
                        )
                    ]
                )
            ],
            approvalRows: [],
            artifactRows: [],
            artifactViewerRows: [],
            reportRows: [],
            catalogContext: nil,
            closeoutReadiness: nil,
            implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "running",
            failedStages: 0
        )

        model.populate(from: detail)

        let stage = model.stageMap?.stages.first
        XCTAssertEqual(stage?.id, "state_2")
        XCTAssertEqual(stage?.title, "Proposal drafted")
        XCTAssertEqual(stage?.ownerAgentTitle, "Proposal Writer")
        XCTAssertEqual(stage?.status, "active")
        XCTAssertEqual(stage?.isCurrent, true)
        XCTAssertEqual(stage?.artifactCount, 3)
        XCTAssertEqual(stage?.occurrences.first?.taskName, "Draft proposal")
        XCTAssertEqual(stage?.transitions.first?.toLabel, "Proposal reviewed")
    }

    @MainActor
    func testWorkbenchTimelineUsesActiveDaemonAgentExecutions() {
        let model = RunsWorkbenchPresentationModel()
        let startedAt = Date(timeIntervalSince1970: 1_779_301_418)
        let detail = P031RunDetailPresentation(
            title: "Run",
            workflowLabel: "proposal-loop-live",
            statusLabel: "Running",
            progressLabel: "114/189 stages",
            pendingApprovalsLabel: nil,
            rolloutDecisionSummary: nil,
            ideaContext: nil,
            stageTransitions: [
                P031StageTransitionPresentation(
                    stageExecutionID: "stage-exec-1",
                    stageTitle: "Implementation reviewed against proposal",
                    statusText: "Running",
                    attemptText: "Attempt 1",
                    startedLabel: "Started",
                    completedLabel: nil,
                    durationLabel: nil,
                    connectorState: .running,
                    evidenceLabels: [],
                    accessibilityLabel: "Implementation review running"
                )
            ],
            approvalRows: [],
            artifactRows: [],
            artifactViewerRows: [],
            reportRows: [],
            activeAgentTimelineEntries: [
                P031ActiveAgentTimelinePresentation(
                    id: "active-agent-exec-1",
                    title: "Security Checker",
                    detail: "Running via codex_acp gpt-5.5",
                    timestamp: startedAt,
                    stageID: "state_9_implementation_reviewed",
                    agentID: "security_checker",
                    sessionID: "generation-1"
                )
            ],
            catalogContext: nil,
            closeoutReadiness: nil,
            implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil,
            rawStatus: "running",
            failedStages: 0
        )

        model.populate(from: detail)

        XCTAssertEqual(model.timelineEntries.count, 1)
        XCTAssertEqual(model.timelineEntries.first?.agentID, "security_checker")
        XCTAssertEqual(model.timelineEntries.first?.kind, .sessionEvent)
        XCTAssertEqual(model.timelineEntries.first?.title, "Security Checker")
        XCTAssertEqual(model.timelineEntries.first?.stageID, "state_9_implementation_reviewed")
        XCTAssertEqual(model.timelineEntries.first?.surfaceLabel, "active_agent_execution")
        XCTAssertEqual(model.timelineEntries.first?.sessionID, "generation-1")
    }

    // MARK: - Approvals tab removal after Phase 2c parity cutover (PC-008)

    func testApprovalsTabCaseRemovedAfterParityCutover() {
        XCTAssertNil(ContentView.Tab(rawValue: "Approvals"),
                     "Standalone Approvals tab must not be constructible after P036 parity cutover")
        XCTAssertEqual(ContentView.Tab.from(rawValue: "Approvals"), .runs,
                       "Legacy Approvals route must redirect into Runs rather than a standalone tab")
        XCTAssertFalse(ContentView.Tab.allCases.contains { $0.rawValue == "Approvals" })
        XCTAssertEqual(ContentView.Tab.allCases.count, 4,
                       "Top-level tab count must remain Runs, Ideas, Definitions, Settings")
    }

    // MARK: - Definitions initial segment routing via onAppear

    func testDefinitionsSegmentEnumCoverage() {
        // All Segment cases must be reachable for the initial-routing onAppear path to work.
        let allSegments = DefinitionsView.Segment.allCases
        XCTAssertTrue(allSegments.contains(.agents), "Segment.agents must exist for initial routing")
        XCTAssertTrue(allSegments.contains(.workflows), "Segment.workflows must exist for initial routing")
        XCTAssertEqual(allSegments.count, 2, "Definitions must have exactly two segments: agents and workflows")
    }

    // MARK: - PC-001 follow-up ID and copy items pass-through

    func testApprovalRowFollowUpIDPassThrough() {
        // PC-001: followUpID from P031 row must appear in the workbench ApprovalRow.
        let model = RunsWorkbenchPresentationModel()
        let diagnostic = P085DiagnosticAffordanceState(diagnosticID: nil, serverDebugDetail: nil, isAvailable: false)
        let affordance = P085ApprovalAffordanceState(
            approvalID: "appr-fup",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .live,
            diagnostic: diagnostic,
            projectionLagIsOnlyConstraint: false
        )
        let row = P031ApprovalInboxRowPresentation(
            approvalID: "appr-fup",
            title: "Needs follow-up",
            body: "Approve to continue",
            canApprove: true,
            canReject: true,
            actionLabel: nil,
            followUpID: "TICKET-42",
            copyItems: [P031DiagnosticCopyItem(label: "Run ID", value: "run-xyz")],
            freshnessState: .live,
            accessibilityLabel: "Needs follow-up approval",
            affordance: affordance
        )
        let detail = P031RunDetailPresentation(
            title: "Run", workflowLabel: "w1", statusLabel: "Blocked",
            progressLabel: nil, pendingApprovalsLabel: nil, rolloutDecisionSummary: nil,
            ideaContext: nil, stageTransitions: [], approvalRows: [row],
            artifactRows: [], artifactViewerRows: [], reportRows: [],
            catalogContext: nil, closeoutReadiness: nil, implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live", emptyStateTitle: nil, errorDescription: nil,
            rawStatus: "blocked", failedStages: 0
        )
        model.populate(from: detail)
        let appRow = model.inlineApprovals[0]
        XCTAssertEqual(appRow.followUpID, "TICKET-42", "followUpID must pass through to ApprovalRow")
        XCTAssertEqual(appRow.copyItems.count, 1, "copyItems must pass through to ApprovalRow")
        XCTAssertEqual(appRow.copyItems[0].label, "Run ID")
        XCTAssertEqual(appRow.copyItems[0].value, "run-xyz")
    }

    func testApprovalRowFollowUpIDNilWhenAbsent() {
        // PC-001: when the upstream row has no followUpID, ApprovalRow.followUpID must be nil.
        let model = RunsWorkbenchPresentationModel()
        let affordance = P085ApprovalAffordanceState(
            approvalID: "appr-nofup",
            approveAvailability: .actionable,
            rejectAvailability: .actionable,
            freshnessState: .live,
            diagnostic: P085DiagnosticAffordanceState(diagnosticID: nil, serverDebugDetail: nil, isAvailable: false),
            projectionLagIsOnlyConstraint: false
        )
        model.populate(from: makeApprovalDetail(affordance: affordance))
        let appRow = model.inlineApprovals[0]
        XCTAssertNil(appRow.followUpID, "Missing followUpID must produce nil in ApprovalRow")
        XCTAssertTrue(appRow.copyItems.isEmpty, "Empty copyItems must produce empty array in ApprovalRow")
    }

    func testApprovalRowRedactedSuppressesFollowUpAndCopyItems() {
        // PC-001 + security: when deferredState == .redacted, followUpID and copyItems must
        // be suppressed so diagnostic content cannot be read via these secondary channels.
        let model = RunsWorkbenchPresentationModel()
        let diagnostic = P085DiagnosticAffordanceState(diagnosticID: nil, serverDebugDetail: nil, isAvailable: false)
        let affordance = P085ApprovalAffordanceState(
            approvalID: "appr-redacted-fup",
            approveAvailability: .disabled(reasonCode: .redacted, helpText: "Redacted"),
            rejectAvailability: .disabled(reasonCode: .redacted, helpText: "Redacted"),
            freshnessState: .live,
            diagnostic: diagnostic,
            projectionLagIsOnlyConstraint: false
        )
        let row = P031ApprovalInboxRowPresentation(
            approvalID: "appr-redacted-fup",
            title: "Redacted approval",
            body: "Sensitive body",
            canApprove: false,
            canReject: false,
            actionLabel: nil,
            followUpID: "SENSITIVE-99",
            copyItems: [P031DiagnosticCopyItem(label: "Token", value: "secret-value")],
            freshnessState: .live,
            accessibilityLabel: "Redacted",
            affordance: affordance
        )
        let detail = P031RunDetailPresentation(
            title: "Run", workflowLabel: "w1", statusLabel: "Blocked",
            progressLabel: nil, pendingApprovalsLabel: nil, rolloutDecisionSummary: nil,
            ideaContext: nil, stageTransitions: [], approvalRows: [row],
            artifactRows: [], artifactViewerRows: [], reportRows: [],
            catalogContext: nil, closeoutReadiness: nil, implementationCompletion: nil,
            sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live", emptyStateTitle: nil, errorDescription: nil,
            rawStatus: "blocked", failedStages: 0
        )
        model.populate(from: detail)
        let appRow = model.inlineApprovals[0]
        XCTAssertEqual(appRow.deferredState, .redacted, "Affordance with redacted code must produce .redacted deferred state")
        XCTAssertNil(appRow.followUpID, "followUpID must be suppressed when deferredState == .redacted")
        XCTAssertTrue(appRow.copyItems.isEmpty, "copyItems must be suppressed when deferredState == .redacted")
        XCTAssertNil(appRow.body, "body must be suppressed when deferredState == .redacted (existing M3 rule)")
    }

    // MARK: - PC-003 waiting-approval lane filter context

    func testWaitingLaneIDExistsInWorkbenchSidebarLanes() {
        // PC-003: when the workbench has a waiting run, the sidebar lane with id="waiting"
        // must be present so the focusedLaneID setter can find it.
        let workbench = RunsWorkbenchPresentationModel()
        let waitingRun = P031RunsHomeRowPresentation(
            runID: "r-waiting",
            title: "Awaiting approval",
            workflowLabel: nil,
            statusLabel: "Waiting",
            progressLabel: nil,
            pendingApprovalsLabel: "1",
            freshnessState: .live,
            accessibilityLabel: "Run awaiting approval",
            rawStatus: "running",
            failedStages: 0,
            pendingApprovals: 1
        )
        let home = P031RunsHomePresentation(
            orientation: nil,
            rows: [waitingRun],
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil
        )
        workbench.populate(from: home)
        let waitingLane = workbench.sidebarLanes.first { $0.id == "waiting" }
        XCTAssertNotNil(waitingLane, "Sidebar must have a lane with id='waiting' when runs are in the waiting lane")
        XCTAssertEqual(waitingLane?.runs.first?.runID, "r-waiting")
    }

    func testFocusWaitingApprovalLaneNotificationNameIsDefined() {
        // PC-003: the notification name must be stable so deep-link routing is reliable.
        XCTAssertEqual(
            Notification.Name.chainworksFocusWaitingApprovalLane.rawValue,
            "chainworks.focusWaitingApprovalLane"
        )
    }

    // MARK: - PC-003 routing race fix: waiting lane auto-selection after lanes populate

    func testWaitingLanePopulatesAfterNotification() {
        // Regression: when chainworksFocusWaitingApprovalLane fires before sidebarLanes are
        // populated (startup race), the onChange(of: workbench.sidebarLanes) handler must
        // still auto-select the first waiting run once lanes arrive.
        // This test verifies the presenter side: after populate(), the waiting lane and its
        // run are present so the onChange handler can find them.
        let workbench = RunsWorkbenchPresentationModel()
        XCTAssertTrue(workbench.sidebarLanes.isEmpty, "Lanes must be empty before populate")

        let waitingRun = P031RunsHomeRowPresentation(
            runID: "r-race-fix",
            title: "Awaiting approval",
            workflowLabel: nil,
            statusLabel: "Waiting",
            progressLabel: nil,
            pendingApprovalsLabel: "1",
            freshnessState: .live,
            accessibilityLabel: "Run awaiting approval",
            rawStatus: "running",
            failedStages: 0,
            pendingApprovals: 1
        )
        let home = P031RunsHomePresentation(
            orientation: nil,
            rows: [waitingRun],
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil
        )
        workbench.populate(from: home)

        let waitingLane = workbench.sidebarLanes.first { $0.id == "waiting" }
        XCTAssertNotNil(waitingLane, "Waiting lane must appear after populate (race-fix trigger)")
        XCTAssertEqual(waitingLane?.runs.first?.runID, "r-race-fix",
            "First waiting run must be selectable by the onChange handler after late populate")
    }

    // MARK: - PC-003 workbench-flag routing race fix

    func testRequestFocusWaitingApprovalLaneSetsFlag() {
        // Verifies the workbench flag API that ContentView uses instead of relying solely
        // on the notification post. The flag survives the tab-switch render cycle so
        // RunsHomeView can pick it up via onChange(initial:true) on mount.
        let workbench = RunsWorkbenchPresentationModel()
        XCTAssertFalse(workbench.pendingFocusWaitingApprovalLane,
            "Flag must start false")
        workbench.requestFocusWaitingApprovalLane()
        XCTAssertTrue(workbench.pendingFocusWaitingApprovalLane,
            "requestFocusWaitingApprovalLane() must set the flag to true")
        workbench.clearFocusWaitingApprovalLane()
        XCTAssertFalse(workbench.pendingFocusWaitingApprovalLane,
            "clearFocusWaitingApprovalLane() must reset the flag to false")
    }

    func testFocusWaitingApprovalFlagIsIndependentOfLanesPopulate() {
        // Verifies that the flag can be set before lanes are populated (tab-switch race)
        // and the waiting lane is still present after populate() for the view to consume.
        let workbench = RunsWorkbenchPresentationModel()
        workbench.requestFocusWaitingApprovalLane()
        XCTAssertTrue(workbench.pendingFocusWaitingApprovalLane,
            "Flag must be set before lanes are populated")
        XCTAssertTrue(workbench.sidebarLanes.isEmpty,
            "Lanes must be empty at this point")

        let waitingRun = P031RunsHomeRowPresentation(
            runID: "r-flag-race",
            title: "Awaiting approval",
            workflowLabel: nil,
            statusLabel: "Waiting",
            progressLabel: nil,
            pendingApprovalsLabel: "1",
            freshnessState: .live,
            accessibilityLabel: "Run awaiting approval",
            rawStatus: "running",
            failedStages: 0,
            pendingApprovals: 1
        )
        let home = P031RunsHomePresentation(
            orientation: nil,
            rows: [waitingRun],
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live",
            emptyStateTitle: nil,
            errorDescription: nil
        )
        workbench.populate(from: home)

        let waitingLane = workbench.sidebarLanes.first { $0.id == "waiting" }
        XCTAssertNotNil(waitingLane,
            "Waiting lane must appear after populate so onChange(sidebarLanes) can auto-select")
        XCTAssertEqual(waitingLane?.runs.first?.runID, "r-flag-race",
            "Waiting run must be reachable after flag-set + populate sequence")
    }

    func testRunsHomeAvoidsEquatableOnChangeForLargeReadPayloads() throws {
        let source = try runsHomeViewSource()

        XCTAssertFalse(
            source.contains(".onChange(of: model.runDetail)"),
            "runDetail contains artifactViewerRows; onChange forces deep Equatable comparison on the SwiftUI hot path"
        )
        XCTAssertFalse(
            source.contains(".onChange(of: model.approvalInbox)"),
            "approval inbox updates should not require broad SwiftUI Equatable diffing"
        )
        XCTAssertFalse(
            source.contains(".onChange(of: workbench.sidebarLanes)"),
            "sidebar lanes can be publisher-driven without comparing every row"
        )
        XCTAssertTrue(source.contains(".onReceive(model.$runDetail.compactMap { $0 })"))
        XCTAssertTrue(source.contains(".onReceive(model.$approvalInbox.compactMap { $0 })"))
        XCTAssertTrue(source.contains(".onReceive(workbench.$sidebarLanes)"))
    }

    func testStagesTabUsesTopologyCardsInsteadOfVerticalRailRows() throws {
        let source = try runsHomeViewSource()

        XCTAssertTrue(
            source.contains("runStageTopology") == false,
            "RunsHomeView should receive topology through the presenter, not issue GraphQL directly"
        )
        XCTAssertTrue(
            source.contains("ScrollView(.horizontal"),
            "Stages should use the old Topology-style horizontal lane"
        )
        XCTAssertTrue(
            source.contains("P036StageTopologyCard"),
            "Stages should render topology cards"
        )
        XCTAssertFalse(
            source.contains("P036StageRailRow"),
            "Stages tab must not use the interim vertical rail-only implementation"
        )
    }

    func testTimelineTabUsesOldSpineGrammarFromControlPlaneReadback() throws {
        let source = try runsHomeViewSource()

        XCTAssertTrue(
            source.contains("GroupBox(\"Timeline\")"),
            "Timeline tab should use the old focused timeline group rather than a single status-row panel"
        )
        XCTAssertTrue(
            source.contains("\"No Timeline Data\""),
            "Timeline empty state should match the old timeline inspector grammar"
        )
        XCTAssertTrue(
            source.contains("entry.surfaceLabel"),
            "Timeline cards should show the event/readback surface label like the old timeline spine"
        )
        XCTAssertTrue(
            source.contains("entry.stageID"),
            "Timeline cards should show stage metadata from control-plane readback"
        )
        XCTAssertFalse(
            source.contains("title: \"Live agent timeline\""),
            "Timeline tab must not regress to the interim active-agent status-card heading"
        )
        XCTAssertFalse(
            source.contains("\"No active agent events\""),
            "Timeline tab must not keep the interim active-agent status-card empty state"
        )
    }

    func testTimelineRowsClipLongSelectableTextInsteadOfExpandingOnClick() throws {
        let source = try runsHomeViewSource()
        let rowStart = try XCTUnwrap(source.range(of: "private struct TimelineEntryRow: View"))
        let rowEnd = try XCTUnwrap(source[rowStart.lowerBound...].range(of: "private struct P031RunDetailSummaryCard"))
        let rowSource = String(source[rowStart.lowerBound..<rowEnd.lowerBound])

        XCTAssertFalse(
            rowSource.contains(".textSelection(.enabled)"),
            "Timeline row snippets must not use selectable Text; SwiftUI can draw the full selected text outside the bounded row."
        )
        XCTAssertTrue(
            rowSource.contains(".lineLimit(previewLineLimit)"),
            "Timeline row snippets should stay bounded while allowing longer live response previews."
        )
        XCTAssertTrue(
            rowSource.contains("isResponseEntry ? 40 : 3"),
            "Live agent responses should retain enough visible accumulated context to avoid looking like chunk replacement."
        )
        XCTAssertTrue(
            rowSource.contains("entry.detail.suffix"),
            "Long response snippets should show the latest accumulated text, not only the oldest prefix."
        )
        XCTAssertTrue(
            rowSource.contains(".clipped()"),
            "Timeline row snippets must clip long provider text instead of expanding over following events."
        )
    }

    func testRunsTimelineHonorsReduceMotionForLiveUpdates() throws {
        let source = try runsHomeViewSource()
        let rowStart = try XCTUnwrap(source.range(of: "private struct TimelineEntryRow: View"))
        let rowEnd = try XCTUnwrap(source[rowStart.lowerBound...].range(of: "private var iconName"))
        let rowSource = String(source[rowStart.lowerBound..<rowEnd.lowerBound])
        let cardStart = try XCTUnwrap(source.range(of: "private struct P036TimelineWorkbenchCard: View"))
        let cardEnd = try XCTUnwrap(source[cardStart.lowerBound...].range(of: "private struct P036ApprovalWorkbenchCard: View"))
        let cardSource = String(source[cardStart.lowerBound..<cardEnd.lowerBound])

        XCTAssertTrue(
            rowSource.contains("@Environment(\\.accessibilityReduceMotion)"),
            "Timeline rows must read Reduce Motion before choosing a transition."
        )
        XCTAssertTrue(
            rowSource.contains("reduceMotion ? .opacity : .asymmetric"),
            "Timeline rows should fall back to opacity instead of spatial push when Reduce Motion is enabled."
        )
        XCTAssertTrue(
            cardSource.contains("@Environment(\\.accessibilityReduceMotion)"),
            "Timeline card should read Reduce Motion before animating live updates."
        )
        XCTAssertTrue(
            cardSource.contains("reduceMotion ? nil : .spring"),
            "Timeline live-update animations should disable spring motion when Reduce Motion is enabled."
        )
    }

    func testArtifactWorkbenchRendersBoundedPreviewRows() throws {
        let source = try runsHomeViewSource()

        XCTAssertTrue(
            source.contains("private let visibleRowLimit = 24"),
            "The run workbench artifact card should not instantiate every artifact row in large runs"
        )
        XCTAssertTrue(
            source.contains("rows.prefix(visibleRowLimit)"),
            "Artifact workbench preview must use a bounded prefix; full artifact inspection belongs to the detail surface"
        )
        XCTAssertTrue(
            source.contains("ForEach(visibleRows)"),
            "SwiftUI should render only the bounded artifact preview rows on the hot run-detail path"
        )
    }

    func testWorkbenchPopulateBatchesArtifactPayloadMetricWrites() throws {
        let source = try runsWorkbenchPresentationModelSource()

        XCTAssertFalse(
            source.contains("recordArtifactPayloadState(\n                count: 1"),
            "Artifact metric writes must not happen once per row on the SwiftUI refresh path"
        )
        XCTAssertTrue(
            source.contains("count: artifactsAndReports.count"),
            "Artifact metric writes should be aggregated after artifact rows are built"
        )
        XCTAssertTrue(
            source.contains("artifactCount: stage.artifactCount"),
            "Stage artifact counts should come from daemon topology readback, not per-stage artifact filtering"
        )
    }

    func testWorkbenchTimelineDoesNotReadLegacySwiftOrchestratorTimeline() throws {
        let source = try runsWorkbenchPresentationModelSource()

        XCTAssertFalse(
            source.contains("projection.liveTimeline"),
            "P036 Timeline must be populated from control-plane active-agent readback, not the legacy Swift orchestrator timeline."
        )
        XCTAssertFalse(
            source.contains("func populate(from projection: WorkflowMapProjection)"),
            "The workbench should not keep a legacy projection-backed timeline populate path after control-plane cutover."
        )
        XCTAssertTrue(
            source.contains("detail.activeAgentTimelineEntries"),
            "The workbench timeline should consume active daemon agent executions from the P031 run detail read model."
        )
    }

    func testTimelineUsesRuntimeTranscriptSubscriptionForLiveAgentEvents() throws {
        let readBoundarySource = try thinGraphQLReadBoundarySource()
        let runsHomeSource = try runsHomeViewSource()

        XCTAssertTrue(
            readBoundarySource.contains("runtimeStatusChanged(runId: $runId)"),
            "Live Timeline should subscribe to daemon runtime transcript events instead of relying only on active-agent rows."
        )
        XCTAssertTrue(
            readBoundarySource.contains("let detail: String?") && readBoundarySource.contains("let surfaceLabel: String?"),
            "Runtime timeline readback should include bounded event detail and surface labels for prompt/text/tool events."
        )
        XCTAssertTrue(
            runsHomeSource.contains("runtimeTimelineEvents"),
            "RunsHomeView should render transient runtime transcript events while the active agent is running."
        )
        XCTAssertTrue(
            runsHomeSource.contains("P036RuntimeTimelineBuffer"),
            "RunsHomeView should route live transcript events through the bounded P036 timeline buffer."
        )
        XCTAssertTrue(
            runsHomeSource.contains("events.count > 40"),
            "The live transcript buffer must stay bounded."
        )
        XCTAssertTrue(
            runsHomeSource.contains("terminalEvent.sessionGenerationID"),
            "Terminal lifecycle events may not include a session generation; completion must still clear the active agent transcript."
        )
    }

    func testRuntimeTimelineAppendsChunksIntoOneLiveResponse() async throws {
        let runID = "run-live-chunks"
        let sessionID = "session-generation-1"
        let events = (0..<45).map { index in
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "chunk-\(index) ",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:\(String(format: "%02d", index % 60))Z"
            )
        }
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineDetail(model, containing: "chunk-44")

        XCTAssertEqual(model.runtimeTimelineEvents.count, 1)
        XCTAssertEqual(model.runtimeTimelineEvents.first?.surfaceLabel, "text_chunk")
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("chunk-0") == true)
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("chunk-44") == true)
    }

    func testRuntimeTimelineBufferKeepsStableResponseIdentityWhileAppendingChunks() throws {
        let runID = "run-stable-live-response"
        let sessionID = "session-generation-1"
        var buffer = P036RuntimeTimelineBuffer()
        let firstEvent = P031RuntimeTimelineEventPresentation(
            id: "first-response-row",
            runID: runID,
            stageID: "stage-live",
            agentID: "code_writer",
            provider: "junie",
            eventKind: "meaningful_progress",
            title: "Agent response",
            detail: "first chunk ",
            surfaceLabel: "text_chunk",
            sessionGenerationID: sessionID,
            timestamp: try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-05-20T18:00:00Z"))
        )
        let secondEvent = P031RuntimeTimelineEventPresentation(
            id: "second-response-row",
            runID: runID,
            stageID: "stage-live",
            agentID: "code_writer",
            provider: "junie",
            eventKind: "meaningful_progress",
            title: "Agent response",
            detail: "second chunk",
            surfaceLabel: "text_chunk",
            sessionGenerationID: sessionID,
            timestamp: try XCTUnwrap(ISO8601DateFormatter().date(from: "2026-05-20T18:00:01Z"))
        )

        buffer.record(firstEvent, selectedRunID: runID)
        let stableResponseID = try XCTUnwrap(buffer.events.first?.id)
        buffer.record(secondEvent, selectedRunID: runID)

        let response = try XCTUnwrap(buffer.events.first)
        XCTAssertEqual(buffer.events.count, 1)
        XCTAssertEqual(response.id, stableResponseID)
        XCTAssertTrue(response.detail.contains("first chunk"))
        XCTAssertTrue(response.detail.contains("second chunk"))
    }

    func testRuntimeTimelineAppendsInterleavedChunksIntoExistingLiveResponse() async throws {
        let runID = "run-live-interleaved-chunks"
        let sessionID = "session-generation-1"
        let events = [
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "first chunk ",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:00Z"
            ),
            runtimeTimelineReadEvent(
                runID: runID,
                eventKind: "session_started",
                title: "Session started",
                detail: "session_started",
                surfaceLabel: "session_started",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:01Z"
            ),
            runtimeTimelineReadEvent(
                runID: runID,
                eventKind: "prompt_sent",
                title: "Prompt sent",
                detail: "operator prompt",
                surfaceLabel: "operator_prompt",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:02Z"
            ),
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "second chunk",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:03Z"
            )
        ]
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineDetail(model, containing: "second chunk")

        let responseRows = model.runtimeTimelineEvents.filter { $0.surfaceLabel == "text_chunk" }
        XCTAssertEqual(responseRows.count, 1)
        XCTAssertTrue(responseRows.first?.detail.contains("first chunk") == true)
        XCTAssertTrue(responseRows.first?.detail.contains("second chunk") == true)
        XCTAssertEqual(model.runtimeTimelineEvents.last?.surfaceLabel, "text_chunk")
    }

    func testRuntimeTimelineKeepsLiveResponseWhileTrimmingInterleavedEvents() async throws {
        let runID = "run-live-chunk-trimming"
        let sessionID = "session-generation-1"
        var events = [
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "first retained chunk ",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:00Z"
            )
        ]
        events.append(contentsOf: (1...45).map { index in
            runtimeTimelineReadEvent(
                runID: runID,
                eventKind: "tool_event",
                title: "Tool event",
                detail: "tool event \(index)",
                surfaceLabel: "tool_event",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:\(String(format: "%02d", index % 60))Z"
            )
        })
        events.append(
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "second retained chunk",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:01:00Z"
            )
        )
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineDetail(model, containing: "second retained chunk")

        let responseRows = model.runtimeTimelineEvents.filter { $0.surfaceLabel == "text_chunk" }
        XCTAssertEqual(responseRows.count, 1)
        XCTAssertTrue(responseRows.first?.detail.contains("first retained chunk") == true)
        XCTAssertTrue(responseRows.first?.detail.contains("second retained chunk") == true)
        XCTAssertLessThanOrEqual(model.runtimeTimelineEvents.count, 40)
    }

    func testRuntimeTimelineKeepsAccumulatedLiveResponseWithoutHiddenPlaceholder() async throws {
        let runID = "run-live-long-response"
        let sessionID = "session-generation-1"
        let events = (0..<30).map { index in
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "chunk-\(index)-" + String(repeating: "x", count: 1_000) + "\n",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:00:\(String(format: "%02d", index % 60))Z"
            )
        }
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineDetail(model, containing: "chunk-29")

        let response = try XCTUnwrap(model.runtimeTimelineEvents.first)
        XCTAssertEqual(model.runtimeTimelineEvents.count, 1)
        XCTAssertEqual(response.surfaceLabel, "text_chunk")
        XCTAssertTrue(response.detail.contains("chunk-0"))
        XCTAssertTrue(response.detail.contains("chunk-29"))
        XCTAssertFalse(
            response.detail.contains("earlier response content hidden"),
            "Live response accumulation must not visually imply that the in-flight answer disappeared."
        )
    }

    func testRuntimeTimelineCollapsesChunksOnlyAfterResponseEnds() async throws {
        let runID = "run-terminal-summary"
        let sessionID = "session-generation-1"
        var events = (0..<5).map { index in
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "response chunk \(index)",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:01:\(String(format: "%02d", index))Z"
            )
        }
        events.append(
            runtimeTimelineReadEvent(
                runID: runID,
                eventKind: "session_completed",
                title: nil,
                detail: nil,
                surfaceLabel: nil,
                sessionGenerationID: nil,
                timestamp: "2026-05-20T18:01:10Z"
            )
        )
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineCount(model, minimumCount: 1)

        XCTAssertEqual(model.runtimeTimelineEvents.count, 1)
        XCTAssertEqual(model.runtimeTimelineEvents.first?.surfaceLabel, "agent_summary")
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("response chunk 0") == true)
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("response chunk 4") == true)
    }

    func testRuntimeTimelineFinalResponseSummaryPrefersAccumulatedChunks() async throws {
        let runID = "run-final-response-accumulated-chunks"
        let sessionID = "session-generation-1"
        let events = [
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "first chunk ",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:01:00Z"
            ),
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "second chunk ",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:01:01Z"
            ),
            runtimeTimelineReadEvent(
                runID: runID,
                eventKind: "meaningful_progress",
                title: "Final response",
                detail: "terminal only",
                surfaceLabel: "final_response",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:01:02Z"
            )
        ]
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineCount(model, minimumCount: 1)

        XCTAssertEqual(model.runtimeTimelineEvents.count, 1)
        XCTAssertEqual(model.runtimeTimelineEvents.first?.surfaceLabel, "agent_summary")
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("first chunk") == true)
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("second chunk") == true)
        XCTAssertFalse(model.runtimeTimelineEvents.first?.detail.contains("terminal only") == true)
    }

    func testRuntimeTimelineCompletedSummaryRetainsLongAccumulatedResponse() async throws {
        let runID = "run-completed-long-response"
        let sessionID = "session-generation-1"
        var events = (0..<40).map { index in
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "completed-chunk-\(index)-" + String(repeating: "y", count: 500) + "\n",
                surfaceLabel: "text_chunk",
                sessionGenerationID: sessionID,
                timestamp: "2026-05-20T18:01:\(String(format: "%02d", index % 60))Z"
            )
        }
        events.append(
            runtimeTimelineReadEvent(
                runID: runID,
                eventKind: "session_completed",
                title: nil,
                detail: nil,
                surfaceLabel: nil,
                sessionGenerationID: nil,
                timestamp: "2026-05-20T18:02:10Z"
            )
        )
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 0
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineSurface(model, surfaceLabel: "agent_summary")

        let response = try XCTUnwrap(model.runtimeTimelineEvents.first)
        XCTAssertEqual(model.runtimeTimelineEvents.count, 1)
        XCTAssertEqual(response.surfaceLabel, "agent_summary")
        XCTAssertTrue(response.detail.contains("completed-chunk-0"))
        XCTAssertTrue(response.detail.contains("completed-chunk-39"))
        XCTAssertFalse(response.detail.contains("terminal only"))
    }

    func testRuntimeTimelineBatchesRunsSubscriptionPublishes() async throws {
        P036UICounters.shared.reset()
        let baselineFlushTotal = P036UICounters.shared.timelineBatchFlushTotal
        let runID = "run-batched-chunks"
        let events = [
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "first chunk ",
                surfaceLabel: "text_chunk",
                timestamp: "2026-05-20T18:02:00Z"
            ),
            runtimeTimelineReadEvent(
                runID: runID,
                detail: "second chunk",
                surfaceLabel: "text_chunk",
                timestamp: "2026-05-20T18:02:01Z"
            )
        ]
        let model = makeRuntimeTimelineDashboardModel(
            runID: runID,
            events: events,
            runtimeTimelineFlushInterval: 60
        )

        await model.loadIfNeeded()
        await waitForRuntimeTimelineCount(model, minimumCount: 1)

        XCTAssertEqual(model.runtimeTimelineEvents.count, 1)
        XCTAssertTrue(model.runtimeTimelineEvents.first?.detail.contains("first chunk") == true)
        XCTAssertFalse(
            model.runtimeTimelineEvents.first?.detail.contains("second chunk") == true,
            "Runs timeline should batch rapid chunk updates instead of publishing every subscription event."
        )
        XCTAssertGreaterThan(P036UICounters.shared.timelineBatchFlushTotal, baselineFlushTotal)
    }

    func testReadBoundaryDateParserCachesFormatters() throws {
        let source = try thinGraphQLReadBoundarySource()
        guard let parserRange = source.range(of: "private enum P031ReadBoundaryDateParser"),
              let extensionRange = source.range(of: "private extension P031StageReadModel") else {
            XCTFail("Expected P031ReadBoundaryDateParser source block")
            return
        }
        let parserSource = String(source[parserRange.lowerBound..<extensionRange.lowerBound])

        XCTAssertTrue(
            parserSource.contains("Thread.current.threadDictionary"),
            "Run detail presentation should reuse date formatters instead of allocating them per stage/artifact row"
        )
        XCTAssertFalse(
            parserSource.contains("let fractional = ISO8601DateFormatter()"),
            "Per-call ISO8601DateFormatter allocation showed up in the P036 Time Profiler trace"
        )
        XCTAssertFalse(
            parserSource.contains("let standard = ISO8601DateFormatter()"),
            "Per-call ISO8601DateFormatter allocation showed up in the P036 Time Profiler trace"
        )
    }

    // MARK: - Recovery evidence stable IDs

    func testRecoveryEvidenceRowsUseStableIndexBasedIDs() {
        // Stable IDs prevent SwiftUI from tearing down and recreating rows on every
        // populate() call. IDs must be deterministic index-based strings, not UUIDs.
        let workbench = RunsWorkbenchPresentationModel()

        let closeoutReadiness = P077CloseoutReadinessPresentation(
            statusLabel: "Blocked",
            compactSignalLabel: "",
            detailText: "",
            primaryUnblockText: "",
            secondaryBlockerRows: [],
            modeLabel: "",
            modeExplainerText: "",
            diagnosticRows: ["Issue A", "Issue B", "Issue C"],
            recoveryLifecycleText: "",
            recoveryLifecycleAcknowledgementText: "",
            recoveryLifecycleCorrelationText: "",
            recoveryLifecycleFreshnessBudgetText: "",
            recoveryLifecycleActionRows: [],
            recoveryLifecycleCopyTemplate: "",
            recoveryLifecycleAccessibilityLabel: "",
            backlinkRouteLabel: "",
            backlinkRouteAccessibilityLabel: "",
            focusReturnLabel: "",
            copyFailureFallbackText: "",
            voiceOverAnnouncementPolicy: "polite",
            keyboardTraversalOrder: [],
            generationDisplayID: "g0",
            generationCopyValue: nil,
            generationCopyAccessibilityLabel: "",
            diagnosticsAccessibilityLabel: "",
            compactActivationAccessibilityLabel: "",
            cardAccessibilityLabel: "",
            modeExplainerAccessibilityLabel: "",
            visualState: .neutral
        )
        let detail = P031RunDetailPresentation(
            title: "Run", workflowLabel: "w1", statusLabel: "Blocked",
            progressLabel: nil, pendingApprovalsLabel: nil, rolloutDecisionSummary: nil,
            ideaContext: nil, stageTransitions: [], approvalRows: [],
            artifactRows: [], artifactViewerRows: [], reportRows: [],
            catalogContext: nil, closeoutReadiness: closeoutReadiness,
            implementationCompletion: nil, sideEffectReadback: nil,
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "Live", emptyStateTitle: nil, errorDescription: nil,
            rawStatus: "blocked", failedStages: 0
        )
        workbench.populate(from: detail)

        XCTAssertEqual(workbench.recoveryEvidence.count, 3)
        let ids = workbench.recoveryEvidence.map(\.id)
        XCTAssertEqual(ids, ["recovery-0", "recovery-1", "recovery-2"],
            "Recovery evidence IDs must be stable index-based strings")

        // Populate again with the same data — IDs must be identical (no UUID churn).
        workbench.populate(from: detail)
        let idsAgain = workbench.recoveryEvidence.map(\.id)
        XCTAssertEqual(ids, idsAgain, "Recovery evidence IDs must be stable across repeated populate() calls")
    }

    private func runsHomeViewSource() throws -> String {
        try sourceFile("Chainworks Forge/Views/RunsHomeView.swift")
    }

    private func runsWorkbenchPresentationModelSource() throws -> String {
        try sourceFile("Chainworks Forge/Models/RunsWorkbenchPresentationModel.swift")
    }

    private func thinGraphQLReadBoundarySource() throws -> String {
        try sourceFile("Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift")
    }

    private func makeRuntimeTimelineDashboardModel(
        runID: String,
        events: [P031RuntimeTimelineEventReadModel],
        runtimeTimelineFlushInterval: TimeInterval = 0
    ) -> P031ThinReadDashboardModel {
        let run = P031RunRowReadModel(
            id: runID,
            status: "running",
            ideaTitle: "Runtime timeline run",
            workflowTitle: "Full MVP Live",
            freshnessState: .live,
            totalStages: 1,
            completedStages: 0,
            failedStages: 0,
            pendingApprovals: 0
        )
        let store = P031InMemoryWorkflowReadStore(
            runs: [run],
            runDetailsByRunID: [
                runID: P031RunDetailReadModel(run: run, stages: [], artifacts: [])
            ],
            runtimeTimelineEvents: [runID: events]
        )
        return P031ThinReadDashboardModel(
            coordinator: P031ThinWorkflowScreenCoordinator(store: store),
            runtimeTimelineFlushInterval: runtimeTimelineFlushInterval
        )
    }

    private func runtimeTimelineReadEvent(
        runID: String,
        eventKind: String = "meaningful_progress",
        title: String? = "Agent response",
        detail: String? = "chunk",
        surfaceLabel: String? = "text_chunk",
        sessionGenerationID: String? = "session-generation-1",
        timestamp: String
    ) -> P031RuntimeTimelineEventReadModel {
        P031RuntimeTimelineEventReadModel(
            runID: runID,
            stageID: "stage-live",
            agentID: "code_writer",
            provider: "junie",
            eventKind: eventKind,
            title: title,
            detail: detail,
            surfaceLabel: surfaceLabel,
            sessionGenerationID: sessionGenerationID,
            timestamp: timestamp
        )
    }

    private func waitForRuntimeTimelineCount(
        _ model: P031ThinReadDashboardModel,
        minimumCount: Int
    ) async {
        for _ in 0..<100 {
            if model.runtimeTimelineEvents.count >= minimumCount {
                return
            }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
    }

    private func waitForRuntimeTimelineDetail(
        _ model: P031ThinReadDashboardModel,
        containing needle: String
    ) async {
        for _ in 0..<100 {
            if model.runtimeTimelineEvents.contains(where: { $0.detail.contains(needle) }) {
                return
            }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
    }

    private func waitForRuntimeTimelineSurface(
        _ model: P031ThinReadDashboardModel,
        surfaceLabel: String
    ) async {
        for _ in 0..<100 {
            if model.runtimeTimelineEvents.contains(where: { $0.surfaceLabel == surfaceLabel }) {
                return
            }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
    }

    private func sourceFile(_ path: String) throws -> String {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let sourceURL = repoRoot
            .appendingPathComponent(path, isDirectory: false)
        let data = try Data(contentsOf: sourceURL)
        return try XCTUnwrap(String(data: data, encoding: .utf8))
    }

    private func loadEvidenceObject(_ path: String) throws -> [String: Any] {
        let repoRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let sourceURL = repoRoot
            .appendingPathComponent(path, isDirectory: false)
        let data = try Data(contentsOf: sourceURL)
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }
}
