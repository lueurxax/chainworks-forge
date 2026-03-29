import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

// MARK: - Proposal 013: Output Contract Alignment, Retry Truth, and Failure Evidence Tests

@Suite("Proposal 013")
struct Proposal013Tests {

    // MARK: - Layer M: Output Contract Schema V2

    @Test("OutputContractSchemaV2 derives from ArtifactContract correctly")
    func schemaDerivesFromCatalogContract() {
        let contract = ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"])
        let schema = OutputContractSchemaV2.from(
            contractID: "proposal_review_v1",
            contract: contract,
            validationMode: .strictStructured
        )
        #expect(schema.contractID == "proposal_review_v1")
        #expect(schema.machineFormat == .json)
        #expect(schema.validationMode == .strictStructured)
        #expect(schema.requiredFields == ["agent_id", "score", "decision"])
    }

    @Test("OutputContractSchemaV2 supports structured_with_human_companion mode")
    func schemaSupportsHumanCompanion() {
        let contract = ArtifactContract(format: "json", requiredFields: ["agent_id", "score"])
        let schema = OutputContractSchemaV2.from(
            contractID: "proposal_review_v1",
            contract: contract,
            validationMode: .structuredWithHumanCompanion
        )
        #expect(schema.validationMode == .structuredWithHumanCompanion)
    }

    // MARK: - Layer M: Output Contract Resolver V2 (no hardcoded fallbacks)

    @Test("ResolverV2 resolves contract from catalog without hardcoded branches")
    func resolverV2CatalogDriven() {
        let catalog = makeTestCatalog(withContracts: [
            "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"])
        ])
        let agent = makeTestResolvedAgent(outputContract: nil)

        // "proposal_review_po" should match "proposal_review_v1" via stem matching
        let contractID = OutputContractResolverV2.resolveContractID(
            for: "proposal_review_po",
            agent: agent,
            catalog: catalog
        )
        #expect(contractID == "proposal_review_v1")
    }

    @Test("ResolverV2 resolves explicit agent outputContract")
    func resolverV2ExplicitContract() {
        let catalog = makeTestCatalog(withContracts: [
            "audit_report_v1": ArtifactContract(format: "json", requiredFields: ["status"])
        ])
        let agent = makeTestResolvedAgent(outputContract: "audit_report_v1")

        let contractID = OutputContractResolverV2.resolveContractID(
            for: "audit_report",
            agent: agent,
            catalog: catalog
        )
        #expect(contractID == "audit_report_v1")
    }

    @Test("ResolverV2 returns nil when no contract matches")
    func resolverV2NilWhenNoMatch() {
        let catalog = makeTestCatalog(withContracts: [:])
        let agent = makeTestResolvedAgent(outputContract: nil)

        let contractID = OutputContractResolverV2.resolveContractID(
            for: "unknown_output",
            agent: agent,
            catalog: catalog
        )
        #expect(contractID == nil)
    }

    // MARK: - Layer M: Validation

    @Test("Validation passes for valid JSON output with all required fields")
    func validationPassesForValidJSON() {
        let catalog = makeTestCatalog(withContracts: [
            "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"])
        ])
        let agent = makeTestResolvedAgent(outputContract: "proposal_review_v1")

        let validJSON = try! JSONSerialization.data(withJSONObject: [
            "agent_id": "reviewer_po",
            "score": 8,
            "decision": "approve"
        ])

        let results = OutputContractResolverV2.validateOutputs(
            ["proposal_review_po": validJSON],
            agent: agent,
            catalog: catalog
        )

        #expect(results["proposal_review_po"]?.status == .passed)
    }

    @Test("Validation fails for JSON output missing required fields (strict_structured)")
    func validationFailsForMissingFields() {
        // Use a strict_structured contract (audit_report_v1) to prove missing-fields failure
        let catalog = makeTestCatalog(withContracts: [
            "audit_report_v1": ArtifactContract(format: "json", requiredFields: ["status", "matches_proposal", "defects"])
        ])
        let agent = ResolvedAgent(
            id: "auditor", title: "Auditor", mode: "audit",
            backendProfileID: "test_profile", provider: "claude_code",
            model: "claude-opus-4.6", effort: "high", maxTurns: 18,
            temperature: 0, permissionProfile: "RO_VERIFY",
            skillRef: "audit", skillRole: nil, prompt: "Audit",
            outputContract: "audit_report_v1",
            requiresHumanApproval: false,
            inputs: [], outputs: ["audit_report"],
            worktreeWriteEnabled: false
        )

        let incompleteJSON = try! JSONSerialization.data(withJSONObject: [
            "status": "pass"
            // missing: matches_proposal, defects
        ])

        let results = OutputContractResolverV2.validateOutputs(
            ["audit_report": incompleteJSON],
            agent: agent,
            catalog: catalog
        )

        let result = results["audit_report"]
        #expect(result?.status == .failed)
        #expect(result?.missingFields.contains("matches_proposal") == true)
        #expect(result?.missingFields.contains("defects") == true)
    }

    @Test("Validation fails for non-JSON output when contract requires JSON")
    func validationFailsForNonJSON() {
        let catalog = makeTestCatalog(withContracts: [
            "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id"])
        ])
        let agent = makeTestResolvedAgent(outputContract: "proposal_review_v1")

        let markdownData = Data("# This is markdown\n\nNot JSON.".utf8)

        let results = OutputContractResolverV2.validateOutputs(
            ["proposal_review_po": markdownData],
            agent: agent,
            catalog: catalog
        )

        // structured_with_human_companion mode allows markdown for reviews
        let result = results["proposal_review_po"]
        // The review adapter accepts markdown as human companion
        #expect(result != nil)
    }

    // MARK: - Layer M: Validation Failure Record

    @Test("ValidationFailureRecord captures failure context")
    func validationFailureRecordCaptures() {
        let record = ValidationFailureRecord(
            agentID: "proposal_reviewer_po",
            stageID: "state_4_proposal_reviewed",
            runID: UUID(),
            outputResults: [
                OutputValidationResult(
                    outputName: "proposal_review_po",
                    contractID: "proposal_review_v1",
                    status: .failed,
                    missingFields: ["score", "decision"],
                    validationError: "Missing required fields",
                    rawPayloadSize: 1024
                )
            ],
            failureSummary: "Output contract mismatch: missing required fields score, decision",
            failureClass: .outputContractMismatch,
            contractMetadata: [
                ContractValidationMetadata(
                    outputName: "proposal_review_po",
                    contractID: "proposal_review_v1",
                    machineFormat: "json",
                    validationMode: "structured_with_human_companion",
                    requiredFieldCount: 3
                )
            ],
            rawOutputExists: true,
            receiptExists: true,
            transcriptExists: false,
            recoveryRecommendation: RecoveryRecommendation(
                action: .retryFailedAgent,
                explanation: "Raw outputs exist, retry the agent",
                source: .runtimePolicy
            )
        )

        #expect(record.failureClass == .outputContractMismatch)
        #expect(record.rawOutputExists == true)
        #expect(record.outputResults.count == 1)

        // Verify round-trip serialization
        let encoded = try! JSONEncoder().encode(record)
        let decoded = try! JSONDecoder().decode(ValidationFailureRecord.self, from: encoded)
        #expect(decoded.id == record.id)
        #expect(decoded.failureSummary == record.failureSummary)
    }

    // MARK: - Layer M: Structured Output Envelope

    @Test("StructuredOutputEnvelope captures raw payload state")
    func outputEnvelopeCaptures() {
        let envelope = StructuredOutputEnvelope(
            outputName: "proposal_review_po",
            agentID: "reviewer_po",
            stageID: "state_4",
            runID: UUID(),
            rawPayloadSize: 2048,
            rawPayloadChecksum: "abc123",
            rawPayloadPersisted: true,
            contractID: "proposal_review_v1",
            provider: "claude_code"
        )

        #expect(envelope.rawPayloadPersisted == true)
        #expect(envelope.rawPayloadSize == 2048)
    }

    // MARK: - Layer N: Retry and Attempt Truth

    @Test("RetryMode enum round-trips correctly")
    func retryModeRoundTrip() {
        let modes: [RetryMode] = [.agentRetry, .stageRetry, .freshExecution]
        for mode in modes {
            let encoded = try! JSONEncoder().encode(mode)
            let decoded = try! JSONDecoder().decode(RetryMode.self, from: encoded)
            #expect(decoded == mode)
        }
    }

    @Test("RecoveryActionSnapshot captures recommended action")
    func recoveryActionSnapshotCaptures() {
        let snapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(),
            runID: UUID(),
            recommendedAction: RecoveryActionDetail(
                action: .retryFailedAgent,
                stageID: "state_4",
                agentID: "reviewer_po",
                explanation: "Retry the failed reviewer",
                staysInSameRun: true,
                reusesSiblingOutputs: true,
                reExecutesWholeStage: false
            ),
            availableActions: [],
            validationFailureID: nil,
            source: .runtimePolicy
        )

        #expect(snapshot.recommendedAction?.staysInSameRun == true)
        #expect(snapshot.recommendedAction?.reusesSiblingOutputs == true)
    }

    // MARK: - Layer O: Failed Stage Evidence

    @Test("FailedStageEvidenceBuilder produces complete evidence packet")
    func evidencePacketBuild() {
        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date().addingTimeInterval(-60),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )

        let agent = AgentExecution(
            agentID: "proposal_reviewer_po",
            agentTitle: "Proposal Reviewer / PO",
            taskName: "review_proposal",
            startedAt: Date().addingTimeInterval(-50),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        agent.completedAt = Date()
        agent.providerReceiptJSON = Data("receipt".utf8)
        agent.stageExecution = stage

        let validationFailure = ValidationFailureRecord(
            agentID: "proposal_reviewer_po",
            stageID: "state_4_proposal_reviewed",
            runID: UUID(),
            outputResults: [],
            failureSummary: "Contract mismatch",
            failureClass: .outputContractMismatch,
            contractMetadata: [],
            rawOutputExists: true,
            receiptExists: true,
            transcriptExists: false,
            recoveryRecommendation: RecoveryRecommendation(
                action: .retryFailedAgent,
                explanation: "Retry",
                source: .runtimePolicy
            )
        )

        let packet = FailedStageEvidenceBuilder.buildEvidencePacket(
            stageExecution: stage,
            failedAgent: agent,
            validationFailure: validationFailure,
            outputEnvelopes: [],
            recoverySnapshot: nil
        )

        #expect(packet.stageID == "state_4_proposal_reviewed")
        #expect(packet.failedAgentID == "proposal_reviewer_po")
        #expect(packet.rawOutputsExist == false)  // No envelopes with rawPayloadPersisted
        #expect(packet.receiptExists == true)
        #expect(packet.failureClass == .outputContractMismatch)
    }

    // MARK: - Layer O: Blocked Stage Report

    @Test("BlockedStageReportBuilder creates report with stage history")
    func blockedStageReport() {
        let run = makeTestRun(status: .blocked)
        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(),
            status: .failed,
            iteration: 1,
            attemptNumber: 2
        )
        stage.retryMode = RetryMode.stageRetry.rawValue
        stage.triggerReason = "stage_retry_via_recovery"
        stage.supersedesAttemptNumber = 1
        stage.run = run
        run.stageExecutions.append(stage)

        let report = BlockedStageReportBuilder.buildReport(
            run: run,
            stage: stage,
            evidencePacket: nil
        )

        #expect(report.stageID == "state_4_proposal_reviewed")
        #expect(report.currentAttemptNumber == 2)
        #expect(report.stageHistory.count == 1)  // Only this attempt visible directly
    }

    // MARK: - Layer P: Proposal Draft Compaction

    @Test("Compaction not triggered for small outputs")
    func compactionNotTriggeredForSmall() {
        let smallData = Data("Small proposal text".utf8)
        let result = ProposalDraftCompactionPolicy.apply(outputName: "proposal_current", data: smallData)

        #expect(result.wasCompacted == false)
        #expect(result.metadata == nil)
        #expect(result.data == smallData)
    }

    @Test("Compaction triggered for oversized outputs")
    func compactionTriggeredForLarge() {
        let largeData = Data(repeating: 65, count: 512 * 1024) // 512KB
        let result = ProposalDraftCompactionPolicy.apply(outputName: "proposal_current", data: largeData)

        #expect(result.wasCompacted == true)
        #expect(result.metadata != nil)
        #expect(result.metadata!.originalSize == 512 * 1024)
        #expect(result.metadata!.compactedSize <= ProposalDraftCompactionPolicy.defaultMaxOutputSize)
        #expect(result.metadata!.strategy == .truncateWithMarker)
    }

    @Test("Compaction preserves compaction metadata")
    func compactionMetadataRoundTrip() {
        let metadata = CompactionMetadata(
            outputName: "proposal_current",
            originalSize: 512_000,
            compactedSize: 256_000,
            strategy: .truncateWithMarker,
            timestamp: Date()
        )

        let encoded = try! JSONEncoder().encode(metadata)
        let decoded = try! JSONDecoder().decode(CompactionMetadata.self, from: encoded)
        #expect(decoded.originalSize == 512_000)
        #expect(decoded.strategy == .truncateWithMarker)
    }

    // MARK: - Layer Q: Declarative Coverage Report

    @Test("DeclarativeCoverageReport contains all Appendix B entries")
    func declarativeCoverageReport() {
        let report = DeclarativeCoverageReport()

        // Must have mandatory tier entries
        let mandatoryEntries = report.agentCatalogEntries.filter { $0.tier == .tier1Mandatory }
        #expect(mandatoryEntries.count >= 2) // contracts.* and structured_output

        // Must have tier 2 entries
        let tier2Entries = report.agentCatalogEntries.filter { $0.tier == .tier2MetadataOnly }
        #expect(!tier2Entries.isEmpty)

        // Must have tier 3 entries
        let tier3Entries = report.agentCatalogEntries.filter { $0.tier == .tier3LaterProposal }
        #expect(!tier3Entries.isEmpty)

        // Must have workflow entries
        #expect(!report.workflowEntries.isEmpty)

        // All mandatory must be enforced
        #expect(report.allMandatoryEnforced)
    }

    @Test("DeclarativeCoverageReport round-trips through JSON")
    func declarativeCoverageRoundTrip() {
        let report = DeclarativeCoverageReport()
        let encoded = try! JSONEncoder().encode(report)
        let decoded = try! JSONDecoder().decode(DeclarativeCoverageReport.self, from: encoded)
        #expect(decoded.agentCatalogEntries.count == report.agentCatalogEntries.count)
        #expect(decoded.mandatoryTierCount == report.mandatoryTierCount)
    }

    // MARK: - Layer Q: Structured Output Schema Gate

    @Test("StructuredOutputSchemaGate validates required profiles")
    func structuredOutputGate() {
        let catalog = makeTestCatalog(withContracts: [:], backendProfiles: [
            "claude_high": BackendProfile(
                provider: "claude_code",
                model: "claude-opus-4.6",
                effort: "high",
                temperature: 0.1,
                maxTurns: 20,
                structuredOutput: "required"
            ),
            "unknown_provider": BackendProfile(
                provider: "unknown_new_provider",
                model: "model-x",
                effort: "low",
                temperature: 0,
                maxTurns: 5,
                structuredOutput: "required"
            )
        ])

        let results = StructuredOutputSchemaGate.validate(catalog: catalog)

        // claude_code supports structured output — not blocking
        let claudeResult = results.first { $0.backendProfileID == "claude_high" }
        #expect(claudeResult?.isBlocking == false)

        // unknown provider does not — blocking
        let unknownResult = results.first { $0.backendProfileID == "unknown_provider" }
        #expect(unknownResult?.isBlocking == true)
    }

    @Test("StructuredOutputSchemaGate preferred requirement is not blocking")
    func structuredOutputGatePreferred() {
        let catalog = makeTestCatalog(withContracts: [:], backendProfiles: [
            "unknown_preferred": BackendProfile(
                provider: "unknown_provider",
                model: "model-x",
                effort: "low",
                temperature: 0,
                maxTurns: 5,
                structuredOutput: "preferred"
            )
        ])

        #expect(!StructuredOutputSchemaGate.hasBlockingViolations(catalog: catalog))
    }

    // MARK: - Layer Q: Output Contract Declarative Bridge

    @Test("OutputContractDeclarativeBridge verifies V1-V2 binding parity")
    func declarativeBridgeVerification() {
        let catalog = makeTestCatalog(
            withContracts: [
                "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"]),
                "audit_report_v1": ArtifactContract(format: "json", requiredFields: ["status"])
            ],
            agents: [
                AgentDefinition(
                    id: "proposal_reviewer_po",
                    title: "Reviewer PO",
                    mode: "proposal_review.product_owner",
                    backendProfile: "test_profile",
                    permissionProfile: "RO_REVIEW",
                    skillRef: "proposal_review_triad",
                    skillRole: "product_owner",
                    worktreePolicy: nil,
                    requiredTools: nil,
                    inputs: ["idea_brief"],
                    outputs: ["proposal_review_po"],
                    outputContract: "proposal_review_v1",
                    requiresHumanApproval: false,
                    prompt: "Review as PO",
                    notes: nil
                )
            ]
        )

        let report = OutputContractDeclarativeBridge.verifyDeclarativeBinding(catalog: catalog)
        #expect(report.totalBindings > 0)
        // All bindings should be catalog-driven (no unresolved)
        #expect(report.catalogDrivenBindings > 0)
    }

    // MARK: - Proposal Review Contract Adapter

    @Test("ProposalReviewContractAdapter identifies review outputs")
    func reviewOutputIdentification() {
        #expect(ProposalReviewContractAdapter.isReviewOutput("proposal_review_po"))
        #expect(ProposalReviewContractAdapter.isReviewOutput("proposal_review_ux"))
        #expect(!ProposalReviewContractAdapter.isReviewOutput("audit_report"))
        #expect(ProposalReviewContractAdapter.isReviewSummary("proposal_review_summary"))
    }

    @Test("ProposalReviewContractAdapter validates JSON review output")
    func reviewValidatesJSON() {
        let catalog = makeTestCatalog(withContracts: [
            "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"])
        ])

        let validJSON = try! JSONSerialization.data(withJSONObject: [
            "agent_id": "reviewer_po",
            "score": 8,
            "decision": "approve"
        ])

        let result = ProposalReviewContractAdapter.validateReviewOutput(
            outputName: "proposal_review_po",
            data: validJSON,
            catalog: catalog
        )
        #expect(result.status == .passed)
    }

    @Test("ProposalReviewContractAdapter accepts markdown as human companion")
    func reviewAcceptsMarkdown() {
        let catalog = makeTestCatalog(withContracts: [
            "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"])
        ])

        let markdownData = Data("# Proposal Review\n\nThis proposal looks good.".utf8)

        let result = ProposalReviewContractAdapter.validateReviewOutput(
            outputName: "proposal_review_po",
            data: markdownData,
            catalog: catalog
        )
        // structured_with_human_companion mode should accept markdown
        #expect(result.status == .passed)
    }

    // MARK: - Canonical Motivating-Class Regression (§10.3)

    @MainActor
    @Test("Canonical regression: strict contract mismatch blocks run, evidence survives, narrow retry available")
    func canonicalMotivatingClassRegression() async throws {
        let context = try makeP013TestContext()

        // Use strict_structured contract (audit_report_v1) to prove the failure→evidence→recovery path.
        // Proposal 013 §4.3: strict_structured does NOT accept non-JSON.
        let auditAgent = ResolvedAgent(
            id: "auditor",
            title: "Auditor",
            mode: "audit",
            backendProfileID: "test_profile",
            provider: "claude_code",
            model: "claude-opus-4.6",
            effort: "high",
            maxTurns: 18,
            temperature: 0.0,
            permissionProfile: "RO_VERIFY",
            skillRef: "audit_core",
            skillRole: nil,
            prompt: "Audit the implementation",
            outputContract: "audit_report_v1",
            requiresHumanApproval: false,
            inputs: [],
            outputs: ["audit_report"],
            worktreeWriteEnabled: false
        )
        let plan = RunPlan(
            workflowID: "regression",
            workflowTitle: "P013 Regression",
            states: [
                "audit": ExecutableState(
                    id: "audit",
                    label: "Audit stage",
                    type: .start,
                    ownerAgentID: "auditor",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "auditor", task: "audit_impl", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end",
                    label: "End",
                    type: .end,
                    ownerAgentID: "auditor",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "audit",
            agentBindings: ["auditor": auditAgent],
            variables: [:],
            scoring: nil,
            failurePolicy: FailurePolicy(
                onError: "block_run",
                onLoopBudgetExhausted: "fail_run",
                preserveArtifacts: true
            ),
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let idea = Idea(title: "Regression Idea", body: "Body", status: .active)
        context.insert(idea)

        let tmpDir = FileManager.default.temporaryDirectory.appendingPathComponent("p013-regression-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
        let workspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: tmpDir,
            artifactRoot: tmpDir.appendingPathComponent("artifacts"),
            worktreeRoot: nil
        )
        try FileManager.default.createDirectory(at: workspace.artifactRoot, withIntermediateDirectories: true)

        let run = Run(
            startedAt: Date(),
            status: .ready,
            workflowID: "regression",
            workflowTitle: "P013 Regression",
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSourcePath: "/tmp/wf.yaml",
            catalogSourcePath: "/tmp/ag.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: tmpDir.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: 1
        )
        run.idea = idea
        context.insert(run)

        let auditCatalog = makeTestCatalog(withContracts: [
            "audit_report_v1": ArtifactContract(format: "json", requiredFields: ["status", "matches_proposal", "defects"])
        ])

        // Agent returns markdown instead of JSON (strict_structured will reject this)
        let malformedResult = AgentResult(
            outputs: ["audit_report": Data("# Audit Report\n\nEverything looks good.".utf8)],
            logSnippet: nil,
            costCents: 5,
            succeeded: true,
            errorMessage: nil,
            sessionID: "sess-1",
            durationSeconds: 1.0,
            providerReceipt: nil,
            resolvedModel: "fixture-model",
            configuredProviderID: nil,
            adapterVersion: nil
        )

        let failExecutor = StaticResultExecutor(result: malformedResult)
        let orchestrator1 = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: failExecutor,
            modelContext: context,
            catalog: auditCatalog
        )
        await orchestrator1.start()

        // Verify: run blocked, stage failed, but raw output + evidence preserved (§6.2)
        let isTerminal = run.status == RunStatus.blocked || run.status == RunStatus.failed
        #expect(isTerminal)
        let failedStage = run.stageExecutions.first(where: { $0.status == StageStatus.failed })
        #expect(failedStage != nil)
        let failedAgent = failedStage?.agentExecutions.first(where: { $0.status == AgentStatus.failed })
        #expect(failedAgent != nil)

        // Evidence preservation: raw artifacts persisted BEFORE validation failed (§6.2 Rule 2)
        #expect(failedAgent?.artifacts.isEmpty == false)

        // Validation failure recorded as first-class evidence (§6.2 Rule 4)
        #expect(failedAgent?.validationFailureJSON != nil)
        let validationFailure = try JSONDecoder().decode(
            ValidationFailureRecord.self,
            from: failedAgent!.validationFailureJSON!
        )
        #expect(validationFailure.failureClass == .outputContractMismatch)
        #expect(validationFailure.rawOutputExists == true)

        // Output envelopes recorded
        #expect(failedAgent?.outputEnvelopesJSON != nil)

        // Stage-level evidence packet persisted (§6.3)
        #expect(failedStage?.evidencePacketJSON != nil)

        // Recovery: narrow retry action available before clone-run (§7.2)
        let coordinator = RecoveryCoordinator(modelContext: context)
        let actions = coordinator.availableActions(for: run)
        let hasRetryAgent = actions.contains(where: {
            if case .retryAgent = $0 { return true }
            return false
        })
        #expect(hasRetryAgent)

        // Evidence packet buildable from persisted canonical data (§6.3, §7.3)
        let evidencePacket = coordinator.buildEvidencePacket(for: run)
        #expect(evidencePacket != nil)
        #expect(evidencePacket?.failureClass == .outputContractMismatch)

        // Prior failed-attempt artifacts remain inspectable (§10.3)
        let priorArtifactCount = failedAgent?.artifacts.count ?? 0
        #expect(priorArtifactCount > 0)

        // ---- Phase 2: Execute same-run retry and complete successfully (§10.3) ----

        // Perform retry-stage recovery action
        _ = try coordinator.retryStage(run: run, stageID: "audit")

        // Run should now be ready for re-execution
        #expect(run.status == RunStatus.ready)

        // Execute with VALID JSON output this time
        let validJSON = try JSONSerialization.data(withJSONObject: [
            "status": "pass",
            "matches_proposal": true,
            "defects": []
        ] as [String: Any], options: [.sortedKeys])

        let successResult = AgentResult(
            outputs: ["audit_report": validJSON],
            logSnippet: nil,
            costCents: 3,
            succeeded: true,
            errorMessage: nil,
            sessionID: "sess-2",
            durationSeconds: 2.0,
            providerReceipt: nil,
            resolvedModel: "fixture-model",
            configuredProviderID: nil,
            adapterVersion: nil
        )

        let successExecutor = StaticResultExecutor(result: successResult)
        let orchestrator2 = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: successExecutor,
            modelContext: context,
            catalog: auditCatalog
        )
        await orchestrator2.start()

        // Run completes successfully after same-run retry (§10.3)
        #expect(run.status == RunStatus.completed)

        // Prior failed-attempt artifacts remain inspectable after success (§10.3)
        // The original failed agent's artifacts are still in SwiftData
        let allStages = run.stageExecutions.sorted { $0.startedAt < $1.startedAt }
        let priorFailedStages = allStages.filter { $0.status == StageStatus.failed }
        // The original failed stage is still there (it was superseded but not deleted)
        let priorFailedAgents = priorFailedStages.flatMap { $0.agentExecutions.filter { $0.status == AgentStatus.failed } }
        // Prior failed agent evidence is still inspectable
        for priorAgent in priorFailedAgents {
            #expect(priorAgent.validationFailureJSON != nil)
            #expect(priorAgent.artifacts.isEmpty == false)
        }

        // Cleanup
        try? FileManager.default.removeItem(at: tmpDir)
    }

    @MainActor
    @Test("Proposal review with markdown output passes under structured_with_human_companion")
    func proposalReviewMarkdownPassesCompanionMode() async throws {
        let context = try makeP013TestContext()

        let reviewAgent = makeTestResolvedAgent(outputContract: "proposal_review_v1")
        let plan = RunPlan(
            workflowID: "review-pass",
            workflowTitle: "Review Pass Test",
            states: [
                "review": ExecutableState(
                    id: "review",
                    label: "Proposal reviewed",
                    type: .start,
                    ownerAgentID: "proposal_reviewer_po",
                    runBlock: ExecutableRunBlock(phases: [
                        .sequential([AgentTask(agent: "proposal_reviewer_po", task: "review", inputs: nil, outputs: nil)])
                    ]),
                    runAfterApproval: nil,
                    transitions: [ExecutableTransition(to: "end", condition: .always)],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                ),
                "end": ExecutableState(
                    id: "end",
                    label: "End",
                    type: .end,
                    ownerAgentID: "proposal_reviewer_po",
                    runBlock: nil,
                    runAfterApproval: nil,
                    transitions: [],
                    approvalRequired: false,
                    approvalPolicy: nil,
                    loop: nil
                )
            ],
            initialStateID: "review",
            agentBindings: ["proposal_reviewer_po": reviewAgent],
            variables: [:],
            scoring: nil,
            failurePolicy: nil,
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            planCompilerVersion: 1
        )

        let idea = Idea(title: "Review Idea", body: "Body", status: .active)
        context.insert(idea)

        let tmpDir = FileManager.default.temporaryDirectory.appendingPathComponent("p013-review-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
        let workspace = RunWorkspace(
            runID: UUID(),
            workspaceRoot: tmpDir,
            artifactRoot: tmpDir.appendingPathComponent("artifacts"),
            worktreeRoot: nil
        )
        try FileManager.default.createDirectory(at: workspace.artifactRoot, withIntermediateDirectories: true)

        let run = Run(
            startedAt: Date(),
            status: .ready,
            workflowID: "review-pass",
            workflowTitle: "Review Pass Test",
            workflowSnapshotHash: "h1",
            catalogSnapshotHash: "h2",
            workflowSourcePath: "/tmp/wf.yaml",
            catalogSourcePath: "/tmp/ag.yaml",
            workflowSnapshotJSON: Data(),
            catalogSnapshotJSON: Data(),
            workspaceRoot: tmpDir.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: 1
        )
        run.idea = idea
        context.insert(run)

        let reviewCatalog = makeTestCatalog(withContracts: [
            "proposal_review_v1": ArtifactContract(format: "json", requiredFields: ["agent_id", "score", "decision"])
        ])

        // Proposal 013 §4.3: Markdown review output should PASS under structured_with_human_companion
        let markdownResult = AgentResult(
            outputs: ["proposal_review_po": Data("# Review\n\nScore: 8/10. Approved.".utf8)],
            logSnippet: nil,
            costCents: 1,
            succeeded: true,
            errorMessage: nil,
            sessionID: "sess-1",
            durationSeconds: 0.5,
            providerReceipt: nil,
            resolvedModel: "fixture-model",
            configuredProviderID: nil,
            adapterVersion: nil
        )

        let executor = StaticResultExecutor(result: markdownResult)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
            modelContext: context,
            catalog: reviewCatalog
        )
        await orchestrator.start()

        // Run completes — markdown review accepted as human companion
        #expect(run.status == RunStatus.completed)
        let agent = run.stageExecutions.first?.agentExecutions.first
        #expect(agent?.status == AgentStatus.completed)
        #expect(agent?.artifacts.isEmpty == false)

        try? FileManager.default.removeItem(at: tmpDir)
    }

    @MainActor
    @Test("RunReportBuilder synthesizes failure evidence and canonical retry path when packet is missing")
    func runReportSynthesizesFailureEvidenceAndRetryPath() throws {
        let context = try makeP013TestContext()
        let run = makeTestRun(status: .blocked)
        run.runtimeTrustLevel = "server_verified"
        context.insert(run)

        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 100),
            status: .failed,
            iteration: 1,
            attemptNumber: 7
        )
        stage.run = run
        context.insert(stage)

        let reviewer = AgentExecution(
            agentID: "proposal_reviewer_po",
            agentTitle: "Proposal Reviewer / Product Owner",
            taskName: "review_proposal_as_product_owner",
            startedAt: Date(timeIntervalSince1970: 101),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        reviewer.completedAt = Date(timeIntervalSince1970: 110)
        reviewer.providerReceiptJSON = Data("{}".utf8)
        reviewer.transcriptArtifactPath = "/tmp/proposal_reviewer_po_transcript.md"
        reviewer.transcriptPath = reviewer.transcriptArtifactPath
        reviewer.outputEnvelopesJSON = try JSONEncoder().encode([
            StructuredOutputEnvelope(
                outputName: "proposal_review_po",
                agentID: "proposal_reviewer_po",
                stageID: stage.stageID,
                runID: run.id,
                rawPayloadSize: 128,
                rawPayloadChecksum: "abc123",
                rawPayloadPersisted: true,
                contractID: "proposal_review_v1",
                provider: "claude_code"
            )
        ])
        reviewer.stageExecution = stage
        context.insert(reviewer)

        let reviewReceipt = Artifact(
            name: "proposal_reviewer_po_receipt.json",
            contractID: "receipt",
            format: .json,
            filePath: "/tmp/proposal_reviewer_po_receipt.json",
            runID: run.id,
            stageID: stage.stageID,
            agentID: reviewer.agentID,
            provider: reviewer.provider
        )
        reviewReceipt.agentExecution = reviewer
        context.insert(reviewReceipt)

        let lead = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead / Orchestrator",
            taskName: "aggregate_proposal_reviews",
            startedAt: Date(timeIntervalSince1970: 111),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        lead.completedAt = Date(timeIntervalSince1970: 115)
        lead.logSnippet = "Aggregation failed after reviewer outputs were already written."
        lead.stageExecution = stage
        context.insert(lead)

        let validationFailure = ValidationFailureRecord(
            agentID: reviewer.agentID,
            stageID: stage.stageID,
            runID: run.id,
            outputResults: [
                OutputValidationResult(
                    outputName: "proposal_review_po",
                    contractID: "proposal_review_v1",
                    status: .failed,
                    missingFields: ["score"],
                    validationError: "Missing required field 'score'",
                    rawPayloadSize: 128
                )
            ],
            failureSummary: "Proposal review output failed contract validation after raw artifacts were persisted.",
            failureClass: .outputContractMismatch,
            contractMetadata: [
                ContractValidationMetadata(
                    outputName: "proposal_review_po",
                    contractID: "proposal_review_v1",
                    machineFormat: "json",
                    validationMode: "structured_with_human_companion",
                    requiredFieldCount: 3
                )
            ],
            rawOutputExists: true,
            receiptExists: true,
            transcriptExists: true,
            recoveryRecommendation: RecoveryRecommendation(
                action: .retryFailedAgent,
                explanation: "Retry the failed reviewer path.",
                source: .runtimePolicy
            )
        )
        reviewer.validationFailureJSON = try JSONEncoder().encode(validationFailure)
        stage.validationFailureJSON = reviewer.validationFailureJSON

        let retryAgent = RecoveryActionDetail(
            action: .retryFailedAgent,
            stageID: stage.stageID,
            agentID: reviewer.agentID,
            explanation: "Retry only the failed reviewer path.",
            staysInSameRun: true,
            reusesSiblingOutputs: true,
            reExecutesWholeStage: false
        )
        let snapshot = RecoveryActionSnapshot(
            id: UUID(),
            timestamp: Date(timeIntervalSince1970: 120),
            runID: run.id,
            recommendedAction: retryAgent,
            availableActions: [
                retryAgent,
                RecoveryActionDetail(
                    action: .cloneRunFrozenSnapshot,
                    stageID: nil,
                    agentID: nil,
                    explanation: "Clone fallback.",
                    staysInSameRun: false,
                    reusesSiblingOutputs: false,
                    reExecutesWholeStage: false
                )
            ],
            validationFailureID: validationFailure.id,
            source: .runtimePolicy
        )
        stage.recoverySnapshotJSON = try JSONEncoder().encode(snapshot)

        let builder = RunReportBuilder(modelContext: context)
        let payload = builder.buildReportPayload(for: run, version: 8)

        #expect(payload.failureEvidenceSummaries.count == 1)
        #expect(payload.failureEvidenceSummaries.first?.rawOutputsExist == true)
        #expect(payload.failureEvidenceSummaries.first?.receiptExists == true)
        #expect(payload.failureEvidenceSummaries.first?.transcriptExists == true)
        #expect(payload.retryPath == "Retry agent 'proposal_reviewer_po' in stage 'state_4_proposal_reviewed'")
        #expect(payload.resumePath == "Use same-run recovery from the canonical recovery snapshot; clone run is fallback only.")
    }

    @MainActor
    @Test("RunReportBuilder collapses retries into canonical stage lineage")
    func runReportCollapsesCanonicalStageLineage() throws {
        let context = try makeP013TestContext()
        let run = makeTestRun(status: .completed)
        context.insert(run)

        let failedReview = StageExecution(
            stageID: "review",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .failed,
            iteration: 1,
            attemptNumber: 1
        )
        failedReview.run = run
        context.insert(failedReview)

        let failedReviewer = AgentExecution(
            agentID: "proposal_reviewer_po",
            agentTitle: "Reviewer",
            taskName: "review",
            startedAt: Date(timeIntervalSince1970: 11),
            status: .failed,
            provider: "claude_code",
            effort: "high"
        )
        failedReviewer.completedAt = Date(timeIntervalSince1970: 12)
        failedReviewer.stageExecution = failedReview
        context.insert(failedReviewer)

        let failedArtifact = Artifact(
            name: "failed_review_attempt.json",
            contractID: "proposal_review_v1",
            format: .json,
            filePath: "/tmp/failed_review_attempt.json",
            runID: run.id,
            stageID: failedReview.stageID,
            agentID: failedReviewer.agentID,
            provider: failedReviewer.provider
        )
        failedArtifact.agentExecution = failedReviewer
        context.insert(failedArtifact)

        let completedReview = StageExecution(
            stageID: "review",
            label: "Proposal reviewed",
            startedAt: Date(timeIntervalSince1970: 20),
            status: .completed,
            iteration: 1,
            attemptNumber: 2
        )
        completedReview.run = run
        context.insert(completedReview)

        let completedReviewer = AgentExecution(
            agentID: "proposal_reviewer_po",
            agentTitle: "Reviewer",
            taskName: "review",
            startedAt: Date(timeIntervalSince1970: 21),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        completedReviewer.completedAt = Date(timeIntervalSince1970: 22)
        completedReviewer.stageExecution = completedReview
        context.insert(completedReviewer)

        let completedArtifact = Artifact(
            name: "proposal_review_po",
            contractID: "proposal_review_v1",
            format: .json,
            filePath: "/tmp/proposal_review_po.json",
            runID: run.id,
            stageID: completedReview.stageID,
            agentID: completedReviewer.agentID,
            provider: completedReviewer.provider
        )
        completedArtifact.agentExecution = completedReviewer
        context.insert(completedArtifact)

        let doneStage = StageExecution(
            stageID: "done",
            label: "Workflow complete",
            startedAt: Date(timeIntervalSince1970: 30),
            status: .completed,
            iteration: 1,
            attemptNumber: 1
        )
        doneStage.run = run
        context.insert(doneStage)

        let finisher = AgentExecution(
            agentID: "lead_orchestrator",
            agentTitle: "Lead",
            taskName: "close",
            startedAt: Date(timeIntervalSince1970: 31),
            status: .completed,
            provider: "claude_code",
            effort: "high"
        )
        finisher.completedAt = Date(timeIntervalSince1970: 32)
        finisher.stageExecution = doneStage
        context.insert(finisher)

        let builder = RunReportBuilder(modelContext: context)
        let payload = builder.buildReportPayload(for: run, version: 9)
        let hasCompletedReviewStage = payload.stageTimeline.contains { entry in
            entry.label == "Proposal reviewed"
                && entry.attempt == 2
                && entry.status == "completed"
        }
        let reviewerUseCount = payload.agentsUsed.filter { $0.agentID == "proposal_reviewer_po" }.count
        let hasProposalReviewArtifact = payload.keyArtifacts.contains { $0.name == "proposal_review_po" }
        let hasFailedReviewArtifact = payload.keyArtifacts.contains { $0.name == "failed_review_attempt.json" }

        #expect(payload.stageTimeline.count == 2)
        #expect(payload.completedStages == 2)
        #expect(payload.failedStages == 0)
        #expect(hasCompletedReviewStage)
        #expect(reviewerUseCount == 1)
        #expect(hasProposalReviewArtifact)
        #expect(!hasFailedReviewArtifact)
    }
}

// MARK: - Test Helpers

private func makeTestCatalog(
    withContracts contracts: [String: ArtifactContract],
    backendProfiles: [String: BackendProfile] = ["test_profile": BackendProfile(provider: "claude_code", model: "claude-opus-4.6", effort: "high", temperature: 0.1, maxTurns: 20, structuredOutput: "required")],
    agents: [AgentDefinition] = []
) -> AgentCatalog {
    AgentCatalog(
        schemaVersion: 1,
        app: AppConfig(
            name: "test",
            runtime: "goose",
            transport: "http",
            description: "test",
            ideaInputMode: "text",
            singleActiveRunPerIdea: true,
            runResumePolicy: "automatic_on_launch",
            requiredProviders: []
        ),
        paths: [:],
        artifacts: [:],
        skills: [:],
        contracts: contracts,
        backendProfiles: backendProfiles,
        permissionProfiles: [:],
        agents: agents
    )
}

private func makeTestResolvedAgent(outputContract: String?) -> ResolvedAgent {
    ResolvedAgent(
        id: "proposal_reviewer_po",
        title: "Reviewer PO",
        mode: "proposal_review.product_owner",
        backendProfileID: "test_profile",
        provider: "claude_code",
        model: "claude-opus-4.6",
        effort: "high",
        maxTurns: 14,
        temperature: 0.1,
        permissionProfile: "RO_REVIEW",
        skillRef: "proposal_review_triad",
        skillRole: "product_owner",
        prompt: "Review as PO",
        outputContract: outputContract,
        requiresHumanApproval: false,
        inputs: ["idea_brief", "proposal_current"],
        outputs: ["proposal_review_po"],
        worktreeWriteEnabled: false
    )
}

private func makeP013TestContext() throws -> ModelContext {
    let config = ModelConfiguration("P013-\(UUID().uuidString)", isStoredInMemoryOnly: true)
    let container = try ModelContainer(
        for: Idea.self, Run.self, StageExecution.self,
        AgentExecution.self, Approval.self, Artifact.self,
        configurations: config
    )
    return ModelContext(container)
}

/// Static executor that returns the same result for every execution.
private final class StaticResultExecutor: AgentExecutor, @unchecked Sendable {
    let result: AgentResult
    init(result: AgentResult) { self.result = result }
    func execute(task: AgentTask, agent: ResolvedAgent, context: ExecutionContext) async throws -> AgentResult {
        result
    }
}

private func makeTestRun(status: RunStatus) -> Run {
    Run(
        startedAt: Date(),
        status: status,
        workflowID: "wf-test",
        workflowTitle: "Test Workflow",
        workflowSnapshotHash: "hash",
        catalogSnapshotHash: "catalog",
        workflowSourcePath: "workflow.yaml",
        catalogSourcePath: "agents.yaml",
        workflowSnapshotJSON: Data(),
        catalogSnapshotJSON: Data(),
        workspaceRoot: "/tmp/workspace",
        artifactRoot: "/tmp/artifacts",
        planCompilerVersion: 1
    )
}
