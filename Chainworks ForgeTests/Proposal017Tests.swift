import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@Suite("Proposal 017")
struct Proposal017Tests {
    @MainActor
    @Test("Swift workflow conflict bridge upserts current blocking conflict")
    func swiftWorkflowConflictBridgeUpsertsCurrentBlockingConflict() throws {
        let run = makeP017Run()
        let initial = makeP017Conflict(
            runID: run.id.uuidString,
            fingerprint: "sha256:p017-same-fingerprint",
            status: .unresolved,
            operatorLabel: "Initial label"
        )
        let updated = makeP017Conflict(
            runID: run.id.uuidString,
            fingerprint: "sha256:p017-same-fingerprint",
            status: .operatorConfirmationRequired,
            operatorLabel: "Updated label"
        )

        run.upsertWorkflowConflictRecord(initial)
        run.upsertWorkflowConflictRecord(updated)

        #expect(run.workflowConflictBridgeV1.conflicts.count == 1)
        #expect(run.currentWorkflowConflictRecord?.status == .operatorConfirmationRequired)
        #expect(run.currentWorkflowConflictRecord?.operatorLabel == "Updated label")
    }

    @MainActor
    @Test("Swift workflow conflict bridge supersedes and resolves current state conflicts")
    func swiftWorkflowConflictBridgeSupersedesAndResolvesCurrentStateConflicts() throws {
        let run = makeP017Run()
        let first = makeP017Conflict(
            runID: run.id.uuidString,
            fingerprint: "sha256:p017-first",
            status: .unresolved,
            operatorLabel: "First conflict"
        )
        let second = makeP017Conflict(
            runID: run.id.uuidString,
            fingerprint: "sha256:p017-second",
            status: .unresolved,
            operatorLabel: "Second conflict"
        )

        run.upsertWorkflowConflictRecord(first)
        run.upsertWorkflowConflictRecord(second)

        let bridgeAfterSupersede = run.workflowConflictBridgeV1
        #expect(bridgeAfterSupersede.conflicts.count == 2)
        #expect(bridgeAfterSupersede.conflicts.first?.status == .superseded)
        #expect(bridgeAfterSupersede.conflicts.first?.supersededByConflictID == second.conflictID)
        #expect(run.currentWorkflowConflictRecord?.conflictFingerprint == second.conflictFingerprint)

        let resolvedCount = run.resolveCurrentWorkflowConflicts(
            currentStateID: "state_4_proposal_reviewed",
            selectedTransitionID: "state_4_proposal_reviewed->state_3_proposal_refinement#0",
            selectedNextStateID: "state_3_proposal_refinement",
            stageExecutionID: UUID(uuidString: "00000000-0000-0000-0000-000000000017"),
            resolvedAt: "2026-04-22T10:02:00Z"
        )

        let bridgeAfterResolve = run.workflowConflictBridgeV1
        #expect(resolvedCount == 1)
        #expect(run.currentWorkflowConflictRecord?.conflictID == nil)
        #expect(bridgeAfterResolve.conflicts.last?.status == .resolved)
        #expect(bridgeAfterResolve.conflicts.last?.resolvedAt == "2026-04-22T10:02:00Z")
        #expect(bridgeAfterResolve.conflicts.last?.resolutionRecordJSON != nil)
    }

    @Test("Swift transition cursor can anchor an unresolved workflow conflict")
    func swiftTransitionCursorAnchorsWorkflowConflictResolutionBoundary() throws {
        let stageExecutionID = UUID(uuidString: "00000000-0000-0000-0000-000000000017")
        let cursor = TransitionCursor.initial().markingWorkflowConflictBlocked(
            currentStateID: "state_4_proposal_reviewed",
            currentStageExecutionID: stageExecutionID
        )

        #expect(cursor.settlementPhase == .awaitingConflictResolution)
        #expect(cursor.lastCompletedStateID == "state_4_proposal_reviewed")
        #expect(cursor.lastCompletedStageExecutionID == stageExecutionID)
        #expect(cursor.nextScheduledStateID == nil)
    }

    @MainActor
    @Test("Swift restart readback requires decision for unresolved workflow conflict")
    func swiftRestartReadbackRequiresDecisionForUnresolvedWorkflowConflict() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let compiler = RunPlanCompiler(modelContext: context)
        let plan = try compiler.previewCompile(
            workflow: loadTestCanonicalWorkflow(),
            catalog: loadTestCanonicalCatalog()
        )
        let run = Run(
            id: workspace.runID,
            startedAt: Date(timeIntervalSince1970: 1_800),
            status: .blocked,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        )
        run.frozenWorkspaceRootPath = workspace.workspaceRoot.path
        run.persistTransitionCursor(
            TransitionCursor.initial().markingWorkflowConflictBlocked(
                currentStateID: "state_4_proposal_reviewed",
                currentStageExecutionID: nil
            )
        )
        run.upsertWorkflowConflictRecord(
            makeP017Conflict(
                runID: run.id.uuidString,
                fingerprint: "sha256:p017-restart-readback",
                status: .unresolved,
                operatorLabel: "Required transition input missing"
            )
        )
        context.insert(run)
        try context.save()

        let actions = try ResumeManager(modelContext: context).classifyInterruptedRuns(compiler: compiler)

        #expect(actions.count == 1)
        switch actions[0] {
        case .needsDecision(let decisionRun, let reason):
            #expect(decisionRun.id == run.id)
            #expect(reason.contains("Workflow conflict requires resolution before resume"))
            #expect(reason.contains("required_artifact_or_field_missing_for_transition"))
        default:
            Issue.record("Expected workflow-conflict needsDecision, got \(actions[0])")
        }
    }

    @MainActor
    @Test("Swift lead-resolved workflow conflict re-enters graph settlement")
    func swiftLeadResolvedWorkflowConflictReentersGraphSettlement() async throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let run = makeP017Run(workspace: workspace)
        run.status = .blocked
        run.persistTransitionCursor(
            TransitionCursor.initial().markingWorkflowConflictBlocked(
                currentStateID: "state_4_proposal_reviewed",
                currentStageExecutionID: nil
            )
        )
        run.upsertWorkflowConflictRecord(
            makeP017Conflict(
                runID: run.id.uuidString,
                fingerprint: "sha256:p017-lead-resolved-continuation",
                status: .leadMediationPending,
                operatorLabel: "Lead mediation selected graph refinement"
            )
        )
        context.insert(run)

        let summary = Data(
            """
            {
              "pass": false,
              "blocker_count": 1
            }
            """.utf8
        )
        let executor = SharedStaticResultExecutor(
            result: AgentResult(
                outputs: ["proposal_review_summary": summary],
                logSnippet: "lead mediation accepted graph transition",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: "session-p017-lead-resolved",
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                outputPresence: .durableOutput
            )
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: makeP017LeadResolvedContinuationPlan(),
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start(from: "state_4_proposal_reviewed")

        #expect(run.currentWorkflowConflictRecord == nil)
        let resolvedConflict = try #require(run.workflowConflictBridgeV1.conflicts.first)
        #expect(resolvedConflict.status == .resolved)
        #expect(resolvedConflict.resolutionRecordJSON != nil)
        #expect(run.status == .waitingApproval)
        #expect(run.transitionCursor?.settlementPhase == .transitionStarted)
        #expect(run.transitionCursor?.lastCompletedStateID == "state_4_proposal_reviewed")
        #expect(run.transitionCursor?.nextScheduledStateID == "state_3_proposal_refinement")
    }

    @Test("Swift terminal-unverifiable cursor carries terminal failure reason")
    func swiftTerminalUnverifiableCursorCarriesTerminalFailureReason() throws {
        let cursor = TransitionCursor.initial().markingTerminal(
            lastCompletedStateID: "state_4_proposal_reviewed",
            terminalFailureReason: "Workflow transition outcome could not be verified"
        )

        #expect(cursor.settlementPhase == .terminal)
        #expect(cursor.lastCompletedStateID == "state_4_proposal_reviewed")
        #expect(cursor.nextScheduledStateID == nil)
        #expect(cursor.terminalFailureReason == "Workflow transition outcome could not be verified")
    }

    @MainActor
    @Test("Swift terminal-unverifiable workflow conflict settles cursor terminal")
    func swiftTerminalUnverifiableWorkflowConflictSettlesCursorTerminal() async throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let run = makeP017Run(workspace: workspace)
        context.insert(run)
        let executor = SharedStaticResultExecutor(
            result: AgentResult(
                outputs: [:],
                logSnippet: "unverifiable transition fixture",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: "session-p017-terminal-unverifiable",
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                outputPresence: .durableOutput
            )
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: makeP017UnverifiablePlan(),
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start()

        let conflict = try #require(run.workflowConflictBridgeV1.conflicts.first)
        #expect(conflict.reason == .workflowConflictUnverifiable)
        #expect(conflict.status == .terminalUnverifiable)
        #expect(conflict.terminalFailureReason == "Workflow transition outcome could not be verified")
        #expect(run.currentWorkflowConflictRecord == nil)
        #expect(run.status == .blocked)
        #expect(run.transitionCursor?.settlementPhase == .terminal)
        #expect(run.transitionCursor?.lastCompletedStateID == "state_unverifiable")
        #expect(run.transitionCursor?.terminalFailureReason == conflict.terminalFailureReason)
    }

    @MainActor
    @Test("Swift run report latest summary carries workflowConflict camelCase wrapper")
    func swiftRunReportLatestSummaryCarriesWorkflowConflictWrapper() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let run = makeP017Run(workspace: workspace)
        context.insert(run)
        run.upsertWorkflowConflictRecord(
            makeP017Conflict(
                runID: run.id.uuidString,
                fingerprint: "sha256:p017-summary",
                status: .unresolved,
                operatorLabel: "Required transition input missing"
            )
        )
        run.implementationHandoffStatus = makeP017ImplementationHandoffStatus(
            runID: run.id.uuidString
        )

        let builder = RunReportBuilder(modelContext: context)
        let payload = builder.buildReportPayload(for: run, version: 17)
        try builder.emitLatestSummary(for: run, basedOn: payload)

        #expect(payload.workflowConflict?.current?.reason == .requiredArtifactOrFieldMissingForTransition)
        #expect(payload.workflowConflict?.candidateTransitionMatrix.first?.result == .missingInput)
        #expect(payload.implementationHandoffStatus?.codeWriterStartStatus == "blocked_before_code")

        let summaryURL = workspace.artifactRoot
            .appendingPathComponent("reports", isDirectory: true)
            .appendingPathComponent("run_summary_latest.json")
        let data = try Data(contentsOf: summaryURL)
        let summary = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let conflict = try #require(summary["workflowConflict"] as? [String: Any])
        let handoff = try #require(summary["implementationHandoffStatus"] as? [String: Any])
        let current = try #require(conflict["current"] as? [String: Any])
        let candidateTransitions = try #require(conflict["candidateTransitionMatrix"] as? [[String: Any]])
        let history = try #require(conflict["history"] as? [[String: Any]])

        #expect(current["reason"] as? String == "required_artifact_or_field_missing_for_transition")
        #expect(current["status"] as? String == "unresolved")
        #expect(current["currentStateID"] as? String == "state_4_proposal_reviewed")
        #expect(conflict["validNextActionClass"] as? String == "await_conflict_resolution")
        #expect(history.count == 1)
        #expect(candidateTransitions.first?["result"] as? String == "missing_input")
        #expect(handoff["status"] as? String == "blocked_before_code")
        #expect(handoff["codeWriterStartStatus"] as? String == "blocked_before_code")
    }

    @MainActor
    @Test("Swift run stores implementation handoff readback separately from agent state")
    func swiftRunStoresImplementationHandoffReadback() throws {
        let run = makeP017Run()
        run.implementationHandoffStatus = makeP017ImplementationHandoffStatus(
            runID: run.id.uuidString
        )

        let status = try #require(run.implementationHandoffStatus)

        #expect(status.schemaVersion == ImplementationHandoffStatus.schemaVersion)
        #expect(status.approvedProposalPresent == false)
        #expect(status.missingInputArtifacts == ["approved_proposal"])
        #expect(status.codeWriterStartStatus == "blocked_before_code")
    }

    @MainActor
    @Test("Swift start_implementation blocks before code_writer when handoff is missing")
    func swiftStartImplementationBlocksBeforeCodeWriterWhenHandoffMissing() async throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let run = makeP017Run(workspace: workspace)
        context.insert(run)
        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: makeP017CodeWriterHandoffPlan(taskName: "start_implementation"),
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start()

        #expect(executor.executedTasks.isEmpty)
        #expect(run.status == .blocked)
        #expect(run.implementationHandoffStatus?.status == "blocked_before_code")
        #expect(run.implementationHandoffStatus?.codeWriterStartStatus == "blocked_before_code")
        #expect(run.currentWorkflowConflictRecord?.reason == .implementationHandoffUnavailable)
    }

    @MainActor
    @Test("Swift start_implementation snapshots proposal_current before code_writer")
    func swiftStartImplementationSnapshotsApprovedProposalBeforeCodeWriter() async throws {
        let (_, context) = try makeTestModelContainer()
        let runID = UUID()
        let workspace = makeTestWorkspace(runID: runID)
        defer { cleanupWorkspace(workspace) }

        let run = makeP017Run(id: runID, workspace: workspace)
        context.insert(run)
        let artifactManager = ArtifactManager(modelContext: context)
        _ = try artifactManager.persistSystemArtifact(
            name: "proposal_current",
            data: Data("# Approved proposal\n".utf8),
            contractID: "proposal_current",
            format: .markdown,
            workspace: workspace,
            stageID: "state_6_proposal_approved",
            agentID: "lead_orchestrator",
            provider: "simulated",
            model: nil,
            effort: nil,
            attemptNumber: 1
        )

        let executor = SimulatedAgentExecutor()
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: makeP017CodeWriterHandoffPlan(taskName: "start_implementation"),
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start()

        #expect(!executor.executedTasks.isEmpty)
        #expect(run.implementationHandoffStatus?.status == "ready")
        #expect(run.implementationHandoffStatus?.codeWriterStartStatus == "running")
        #expect(run.implementationHandoffStatus?.approvedProposalPresent == true)
        #expect(run.implementationHandoffStatus?.approvedProposalArtifactID != nil)
        #expect(run.implementationHandoffStatus?.approvedProposalDigest != nil)
    }

    @MainActor
    @Test("Swift graph transition persists non-blocking advisory rejection")
    func swiftGraphTransitionPersistsNonBlockingAdvisoryRejection() async throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let run = makeP017Run(workspace: workspace)
        context.insert(run)

        let summary = Data(
            """
            {
              "pass": false,
              "blocker_count": 1,
              "next_action": "revise_proposal",
              "next_stage": "state_3_proposal_drafted"
            }
            """.utf8
        )
        let executor = SharedStaticResultExecutor(
            result: AgentResult(
                outputs: ["proposal_review_summary": summary],
                logSnippet: "review summary",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: "session-p017",
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                outputPresence: .durableOutput
            )
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: makeP017AdvisoryReplayPlan(),
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(run.currentWorkflowConflictRecord == nil)

        let rejection = try #require(run.workflowConflictBridgeV1.advisoryRejections.first)
        #expect(rejection.currentStateID == "state_4_proposal_reviewed")
        #expect(rejection.selectedNextStateID == "state_3_proposal_refinement")
        #expect(rejection.advisoryNextStageHint == "state_3_proposal_drafted")
        #expect(rejection.advisoryNextAction == "revise_proposal")
        #expect(rejection.graphMembershipResult == "absent_from_graph")
        #expect(rejection.advisoryHintProvenance.map(\.advisoryPath).sorted() == ["$.next_action", "$.next_stage"])
    }

    @MainActor
    @Test("Swift graph transition consumes projected run_state advisory evidence")
    func swiftGraphTransitionConsumesProjectedRunStateAdvisoryEvidence() async throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        defer { cleanupWorkspace(workspace) }

        let advisoryURL = workspace.artifactRoot
            .appendingPathComponent("p017-superseded-run-state.json")
        try FileManager.default.createDirectory(
            at: advisoryURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data(
            """
            {"next_stage":"state_3_proposal_drafted","next_action":"revise_proposal"}
            """.utf8
        ).write(to: advisoryURL)

        let run = makeP017Run(workspace: workspace)
        context.insert(run)

        let summary = Data(
            """
            {
              "pass": false,
              "blocker_count": 1
            }
            """.utf8
        )
        let projection = Data(
            """
            {
              "contract_id": "run_state_projection_v1",
              "active_index": {
                "advisory_artifacts": [
                  {
                    "advisory_id": "superseded-run-state",
                    "advisory_path": "\(advisoryURL.path)",
                    "source_agent_execution_id": "agent-exec-review-1"
                  }
                ]
              }
            }
            """.utf8
        )
        let executor = SharedStaticResultExecutor(
            result: AgentResult(
                outputs: [
                    "proposal_review_summary": summary,
                    "run_state_projection": projection
                ],
                logSnippet: "review summary",
                costCents: nil,
                succeeded: true,
                errorMessage: nil,
                sessionID: "session-p017-projection",
                durationSeconds: 0,
                providerReceipt: nil,
                resolvedModel: nil,
                configuredProviderID: nil,
                adapterVersion: nil,
                outputPresence: .durableOutput
            )
        )
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: makeP017AdvisoryReplayPlan(outputs: [
                "proposal_review_summary",
                "run_state_projection"
            ]),
            workspace: workspace,
            executor: executor,
            modelContext: context
        )

        await orchestrator.start()

        #expect(run.status == .completed)
        #expect(run.currentWorkflowConflictRecord == nil)

        let rejection = try #require(run.workflowConflictBridgeV1.advisoryRejections.first)
        #expect(rejection.selectedNextStateID == "state_3_proposal_refinement")
        #expect(rejection.advisoryNextStageHint == "state_3_proposal_drafted")
        #expect(rejection.advisoryNextAction == "revise_proposal")
        #expect(rejection.advisoryHintProvenance.allSatisfy { $0.supersededByProjection })
        #expect(rejection.advisoryHintProvenance.first?.sourceAgentExecutionID == "agent-exec-review-1")
    }

    @Test("Swift candidate evaluation distinguishes undeclared artifacts from missing inputs")
    func swiftCandidateEvaluationDistinguishesUnknownAndMissingInputs() {
        let context = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: ["proposal_review_summary"],
            declaredArtifactNames: ["proposal_review_summary", "proposal_refinement_plan"],
            approvalGranted: false,
            approvalRejected: false,
            variables: [:],
            artifactFields: [:]
        )
        let transitions = [
            ExecutableTransition(
                to: "state_unknown",
                condition: .expression("exists('unknown_artifact')")
            ),
            ExecutableTransition(
                to: "state_missing_artifact",
                condition: .expression("exists('proposal_refinement_plan')")
            ),
            ExecutableTransition(
                to: "state_missing_field",
                condition: .expression("proposal_review_summary.pass == false")
            )
        ]

        let evaluations = TransitionEvaluator.evaluateCandidates(
            transitions: transitions,
            fromStateID: "state_4_proposal_reviewed",
            context: context
        )

        #expect(evaluations[0].result == .invalidExpression)
        #expect(evaluations[0].requiredArtifacts == ["unknown_artifact"])
        #expect(evaluations[0].sanitizedDiagnostic?.contains("not declared") == true)

        #expect(evaluations[1].result == .missingInput)
        #expect(evaluations[1].requiredArtifacts == ["proposal_refinement_plan"])
        #expect(evaluations[1].missingArtifacts == ["proposal_refinement_plan"])

        #expect(evaluations[2].result == .missingInput)
        #expect(evaluations[2].requiredArtifacts == ["proposal_review_summary"])
        #expect(evaluations[2].missingFields == ["proposal_review_summary.pass"])
    }

    @Test("Swift authority resolution selects only one graph transition")
    func swiftAuthorityResolutionSelectsSingleTransition() {
        let context = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: ["proposal_review_summary"],
            declaredArtifactNames: ["proposal_review_summary"],
            approvalGranted: false,
            approvalRejected: false,
            variables: [:],
            artifactFields: [
                "proposal_review_summary": ["pass": .bool(false)]
            ]
        )

        let resolution = TransitionEvaluator.resolveAuthority(
            transitions: [
                ExecutableTransition(
                    to: "state_refine",
                    condition: .expression("proposal_review_summary.pass == false")
                ),
                ExecutableTransition(
                    to: "state_approved",
                    condition: .expression("proposal_review_summary.pass == true")
                )
            ],
            fromStateID: "state_reviewed",
            context: context
        )

        #expect(resolution.selectedNextStateID == "state_refine")
        #expect(resolution.conflictReason == nil)
        #expect(resolution.candidateEvaluations.map(\.result) == [.matched, .notMatched])
    }

    @Test("Swift authority resolution blocks ambiguous and unverifiable transitions")
    func swiftAuthorityResolutionBlocksAmbiguousAndUnverifiableTransitions() {
        let context = TransitionEvaluator.EvaluationContext(
            producedArtifactNames: [],
            declaredArtifactNames: ["known_artifact"],
            approvalGranted: false,
            approvalRejected: false,
            variables: [:],
            artifactFields: [:]
        )

        let multiMatch = TransitionEvaluator.resolveAuthority(
            transitions: [
                ExecutableTransition(to: "state_a", condition: .always),
                ExecutableTransition(to: "state_b", condition: .expression("true"))
            ],
            fromStateID: "state_reviewed",
            context: context
        )

        let missingInput = TransitionEvaluator.resolveAuthority(
            transitions: [
                ExecutableTransition(
                    to: "state_waiting",
                    condition: .expression("exists('known_artifact')")
                )
            ],
            fromStateID: "state_reviewed",
            context: context
        )

        let unverifiable = TransitionEvaluator.resolveAuthority(
            transitions: [
                ExecutableTransition(
                    to: "state_unknown",
                    condition: .expression("exists('unknown_artifact')")
                )
            ],
            fromStateID: "state_reviewed",
            context: context
        )

        #expect(multiMatch.selectedNextStateID == nil)
        #expect(multiMatch.conflictReason == .multipleDeclarativeTransitionsMatchedWithoutTieBreak)

        #expect(missingInput.selectedNextStateID == nil)
        #expect(missingInput.conflictReason == .requiredArtifactOrFieldMissingForTransition)
        #expect(missingInput.candidateEvaluations.first?.result == .missingInput)

        let mixedUnverifiable = TransitionEvaluator.resolveAuthority(
            transitions: [
                ExecutableTransition(
                    to: "state_waiting",
                    condition: .expression("exists('known_artifact')")
                ),
                ExecutableTransition(
                    to: "state_unknown",
                    condition: .expression("exists('unknown_artifact')")
                )
            ],
            fromStateID: "state_reviewed",
            context: context
        )
        #expect(unverifiable.selectedNextStateID == nil)
        #expect(unverifiable.conflictReason == .workflowConflictUnverifiable)
        #expect(unverifiable.candidateEvaluations.first?.result == .invalidExpression)

        #expect(mixedUnverifiable.selectedNextStateID == nil)
        #expect(mixedUnverifiable.conflictReason == .workflowConflictUnverifiable)
        #expect(mixedUnverifiable.candidateEvaluations.map(\.result) == [.missingInput, .invalidExpression])
    }
}

private func makeP017Run(id: UUID = UUID(), workspace: RunWorkspace? = nil) -> Run {
    Run(
        id: id,
        startedAt: Date(timeIntervalSince1970: 1_800),
        status: .blocked,
        workflowID: "wf-p017",
        workflowTitle: "P017 Workflow",
        workflowSnapshotHash: "workflow-hash",
        catalogSnapshotHash: "catalog-hash",
        workflowSourcePath: "workflow.yaml",
        catalogSourcePath: "agents.yaml",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        workspaceRoot: workspace?.workspaceRoot.path ?? "/tmp/p017-workspace",
        artifactRoot: workspace?.artifactRoot.path ?? "/tmp/p017-artifacts",
        planCompilerVersion: 1
    )
}

private func makeP017Conflict(
    runID: String,
    fingerprint: String,
    status: WorkflowConflictStatus,
    operatorLabel: String
) -> WorkflowConflictRecord {
    let candidate = CandidateTransitionEvaluation(
        transitionID: "review_failed_refine",
        fromStateID: "state_4_proposal_reviewed",
        toStateID: "state_3_proposal_refinement",
        conditionExpressionID: "proposal_review_summary.pass == false",
        result: .missingInput,
        requiredArtifacts: ["proposal_review_summary"],
        missingArtifacts: ["proposal_review_summary"],
        missingFields: ["pass"],
        sourceArtifactIDs: [],
        sourceAgentExecutionID: nil,
        sanitizedDiagnostic: "proposal_review_summary.pass is required"
    )
    return WorkflowConflictRecord(
        conflictID: "conflict-\(fingerprint)",
        conflictFingerprint: fingerprint,
        runID: runID,
        stageExecutionID: nil,
        lineageID: nil,
        currentStateID: "state_4_proposal_reviewed",
        reason: .requiredArtifactOrFieldMissingForTransition,
        operatorLabel: operatorLabel,
        status: status,
        candidateTransitions: [candidate],
        candidateTransitionHash: "sha256:candidate",
        advisoryEvidenceRefs: ["artifact:proposal_review_summary:next_stage"],
        createdAt: "2026-04-22T10:00:00Z",
        updatedAt: "2026-04-22T10:01:00Z"
    )
}

private func makeP017ImplementationHandoffStatus(runID: String) -> ImplementationHandoffStatus {
    ImplementationHandoffStatus(
        runID: runID,
        currentStateID: "state_7_implementation_started",
        taskName: "initial_implementation",
        requiredInputArtifacts: ["approved_proposal"],
        availableInputArtifacts: [],
        missingInputArtifacts: ["approved_proposal"],
        approvedProposalPresent: false,
        worktreeRoot: "/tmp/p017-worktree",
        workspaceRoot: "/tmp/p017-workspace",
        artifactRoot: "/tmp/p017-artifacts",
        codeWriterStartStatus: "blocked_before_code",
        status: "blocked_before_code",
        missingHandoffOutputs: ["approved_proposal"],
        retryableFrom: "implementation_handoff:state_7_implementation_started",
        blockedBeforeCodeReason: "implementation_handoff_unavailable",
        updatedAt: "2026-04-22T10:03:00Z"
    )
}

private func makeP017CodeWriterHandoffPlan(taskName: String) -> RunPlan {
    let codeWriter = ResolvedAgent(
        id: "code_writer",
        title: "Code Writer",
        mode: "tool_use",
        provider: "simulated",
        model: "fixture",
        effort: "low",
        maxTurns: 1,
        temperature: 0,
        permissionProfile: "",
        skillRef: "",
        skillRole: nil,
        prompt: "",
        outputContract: nil,
        requiresHumanApproval: false,
        inputs: ["approved_proposal"],
        outputs: ["implementation_self_assessment"]
    )
    return RunPlan(
        workflowID: "wf-p017-code-writer-handoff",
        workflowTitle: "P017 Code Writer Handoff",
        states: [
            "state_7_implementation_started": ExecutableState(
                id: "state_7_implementation_started",
                label: "Implementation Started",
                type: .start,
                ownerAgentID: "code_writer",
                runBlock: ExecutableRunBlock(phases: [
                    .sequential([
                        AgentTask(
                            agent: "code_writer",
                            task: taskName,
                            inputs: ["approved_proposal"],
                            outputs: ["implementation_self_assessment"]
                        )
                    ])
                ]),
                runAfterApproval: nil,
                transitions: [
                    ExecutableTransition(to: "done", condition: .always)
                ],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            ),
            "done": ExecutableState(
                id: "done",
                label: "Done",
                type: .end,
                ownerAgentID: "code_writer",
                runBlock: nil,
                runAfterApproval: nil,
                transitions: [],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            )
        ],
        initialStateID: "state_7_implementation_started",
        agentBindings: ["code_writer": codeWriter],
        variables: [:],
        scoring: nil,
        failurePolicy: nil,
        workflowSnapshotHash: "workflow-hash",
        catalogSnapshotHash: "catalog-hash",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        planCompilerVersion: 1
    )
}

private func makeP017UnverifiablePlan() -> RunPlan {
    let agent = ResolvedAgent(
        id: "transition_probe",
        title: "Transition Probe",
        mode: "tool_use",
        provider: "simulated",
        model: "fixture",
        effort: "low",
        maxTurns: 1,
        temperature: 0,
        permissionProfile: "",
        skillRef: "",
        skillRole: nil,
        prompt: "",
        outputContract: nil,
        requiresHumanApproval: false,
        inputs: [],
        outputs: []
    )
    return RunPlan(
        workflowID: "wf-p017-terminal-unverifiable",
        workflowTitle: "P017 Terminal Unverifiable",
        states: [
            "state_unverifiable": ExecutableState(
                id: "state_unverifiable",
                label: "Unverifiable",
                type: .start,
                ownerAgentID: "transition_probe",
                runBlock: ExecutableRunBlock(phases: [
                    .sequential([
                        AgentTask(
                            agent: "transition_probe",
                            task: "probe_transition",
                            inputs: nil,
                            outputs: []
                        )
                    ])
                ]),
                runAfterApproval: nil,
                transitions: [
                    ExecutableTransition(
                        to: "done",
                        condition: .expression("exists('unknown_artifact')")
                    )
                ],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            ),
            "done": ExecutableState(
                id: "done",
                label: "Done",
                type: .end,
                ownerAgentID: "transition_probe",
                runBlock: nil,
                runAfterApproval: nil,
                transitions: [],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            )
        ],
        initialStateID: "state_unverifiable",
        agentBindings: ["transition_probe": agent],
        variables: [:],
        scoring: nil,
        failurePolicy: nil,
        workflowSnapshotHash: "workflow-hash",
        catalogSnapshotHash: "catalog-hash",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        planCompilerVersion: 1
    )
}

private func makeP017AdvisoryReplayPlan(
    outputs: [String] = ["proposal_review_summary"]
) -> RunPlan {
    let reviewAgent = ResolvedAgent(
        id: "review_aggregator",
        title: "Review Aggregator",
        mode: "tool_use",
        provider: "simulated",
        model: "fixture",
        effort: "low",
        maxTurns: 1,
        temperature: 0,
        permissionProfile: "",
        skillRef: "",
        skillRole: nil,
        prompt: "",
        outputContract: nil,
        requiresHumanApproval: false,
        inputs: [],
        outputs: outputs
    )
    return RunPlan(
        workflowID: "wf-p017-advisory",
        workflowTitle: "P017 Advisory Replay",
        states: [
            "state_4_proposal_reviewed": ExecutableState(
                id: "state_4_proposal_reviewed",
                label: "Proposal Reviewed",
                type: .start,
                ownerAgentID: "review_aggregator",
                runBlock: ExecutableRunBlock(phases: [
                    .sequential([
                        AgentTask(
                            agent: "review_aggregator",
                            task: "aggregate_review",
                            inputs: nil,
                            outputs: outputs
                        )
                    ])
                ]),
                runAfterApproval: nil,
                transitions: [
                    ExecutableTransition(
                        to: "state_3_proposal_refinement",
                        condition: .expression("proposal_review_summary.pass == false")
                    ),
                    ExecutableTransition(
                        to: "state_5_proposal_approved",
                        condition: .expression("proposal_review_summary.pass == true")
                    )
                ],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            ),
            "state_3_proposal_refinement": ExecutableState(
                id: "state_3_proposal_refinement",
                label: "Proposal Refinement",
                type: .end,
                ownerAgentID: "review_aggregator",
                runBlock: nil,
                runAfterApproval: nil,
                transitions: [],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            ),
            "state_5_proposal_approved": ExecutableState(
                id: "state_5_proposal_approved",
                label: "Proposal Approved",
                type: .end,
                ownerAgentID: "review_aggregator",
                runBlock: nil,
                runAfterApproval: nil,
                transitions: [],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            )
        ],
        initialStateID: "state_4_proposal_reviewed",
        agentBindings: ["review_aggregator": reviewAgent],
        variables: [:],
        scoring: nil,
        failurePolicy: nil,
        workflowSnapshotHash: "workflow-hash",
        catalogSnapshotHash: "catalog-hash",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        planCompilerVersion: 1
    )
}

private func makeP017LeadResolvedContinuationPlan() -> RunPlan {
    let reviewAgent = ResolvedAgent(
        id: "review_aggregator",
        title: "Review Aggregator",
        mode: "tool_use",
        provider: "simulated",
        model: "fixture",
        effort: "low",
        maxTurns: 1,
        temperature: 0,
        permissionProfile: "",
        skillRef: "",
        skillRole: nil,
        prompt: "",
        outputContract: nil,
        requiresHumanApproval: false,
        inputs: [],
        outputs: ["proposal_review_summary"]
    )
    return RunPlan(
        workflowID: "wf-p017-lead-resolved-continuation",
        workflowTitle: "P017 Lead Resolved Continuation",
        states: [
            "state_4_proposal_reviewed": ExecutableState(
                id: "state_4_proposal_reviewed",
                label: "Proposal Reviewed",
                type: .start,
                ownerAgentID: "review_aggregator",
                runBlock: ExecutableRunBlock(phases: [
                    .sequential([
                        AgentTask(
                            agent: "review_aggregator",
                            task: "aggregate_review",
                            inputs: nil,
                            outputs: ["proposal_review_summary"]
                        )
                    ])
                ]),
                runAfterApproval: nil,
                transitions: [
                    ExecutableTransition(
                        to: "state_3_proposal_refinement",
                        condition: .expression("proposal_review_summary.pass == false")
                    )
                ],
                approvalRequired: false,
                approvalPolicy: nil,
                loop: nil
            ),
            "state_3_proposal_refinement": ExecutableState(
                id: "state_3_proposal_refinement",
                label: "Proposal Refinement",
                type: .start,
                ownerAgentID: "review_aggregator",
                runBlock: nil,
                runAfterApproval: nil,
                transitions: [],
                approvalRequired: true,
                approvalPolicy: "lead_resolved_continuation_fixture",
                loop: nil
            )
        ],
        initialStateID: "state_4_proposal_reviewed",
        agentBindings: ["review_aggregator": reviewAgent],
        variables: [:],
        scoring: nil,
        failurePolicy: nil,
        workflowSnapshotHash: "workflow-hash",
        catalogSnapshotHash: "catalog-hash",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        planCompilerVersion: 1
    )
}
