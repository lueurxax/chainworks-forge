import Testing
import Foundation
import SwiftData
@testable import Chainworks_Forge

@MainActor
@Suite("Proposal 025", .serialized)
struct Proposal025Tests {
    @Test("Portable MCP registry fixture overrides unreadable inherited config path")
    func portableMCPRegistryFixtureOverridesUnreadableInheritedPath() throws {
        let envKey = CodexExtensionRegistryReader.environmentConfigPathKey
        let original = ProcessInfo.processInfo.environment[envKey]
        let unreadablePath = FileManager.default.temporaryDirectory
            .appendingPathComponent("missing-mcp-\(UUID().uuidString).yaml", isDirectory: false)
            .path

        setenv(envKey, unreadablePath, 1)
        defer {
            if let original {
                setenv(envKey, original, 1)
            } else {
                unsetenv(envKey)
            }
        }

        _ = try makeTestModelContext()

        let resolved = ProcessInfo.processInfo.environment[envKey]
        let expectedSuffix = "/examples/mcp/mcp-config-fixture.yaml"
        #expect(resolved?.hasSuffix(expectedSuffix) == true)
        #expect(resolved != unreadablePath)
    }

    @Test("Codex registry reader falls back to repo fixture on test hosts")
    func codexRegistryReaderFallsBackToRepoFixtureOnTestHosts() {
        let envKey = CodexExtensionRegistryReader.environmentConfigPathKey
        let original = ProcessInfo.processInfo.environment[envKey]
        unsetenv(envKey)
        defer {
            if let original {
                setenv(envKey, original, 1)
            } else {
                unsetenv(envKey)
            }
        }

        let reader = CodexExtensionRegistryReader()
        #expect(reader.configURL.path.hasSuffix("/examples/mcp/mcp-config-fixture.yaml"))
    }

    @Test("Simulated canonical contract outputs satisfy bundled workflow thresholds")
    func simulatedCanonicalContractOutputsSatisfyBundledWorkflowThresholds() throws {
        let summaryData = OutputContractTemplates.generate(
            contractID: "proposal_review_summary_v2",
            agentID: "lead_orchestrator",
            stageID: "state_4_proposal_reviewed"
        ).data
        let auditData = OutputContractTemplates.generate(
            contractID: "audit_report_v1",
            agentID: "security_checker",
            stageID: "state_9_implementation_reviewed"
        ).data
        let implementationReviewData = OutputContractTemplates.generate(
            contractID: "implementation_review_summary_v1",
            agentID: "lead_orchestrator",
            stageID: "state_9_implementation_reviewed"
        ).data

        let summary = try JSONSerialization.jsonObject(with: summaryData) as? [String: Any]
        let audit = try JSONSerialization.jsonObject(with: auditData) as? [String: Any]
        let implementationReview = try JSONSerialization.jsonObject(with: implementationReviewData) as? [String: Any]

        #expect((summary?["average_score"] as? Double ?? 0) > 9.1)
        #expect((summary?["min_individual_score"] as? Double ?? 0) >= 8.5)
        #expect((summary?["blocker_count"] as? Int) == 0)
        #expect((audit?["status"] as? String) == "Implemented")
        #expect((implementationReview?["status"] as? String) == "Implemented")

        let connectUploadReceiptData = OutputContractTemplates.generate(
            contractID: "connect_upload_receipt_v1",
            agentID: "build_archive_and_push_connect",
            stageID: "state_11_manual_release"
        ).data
        let finalFeatureReportData = OutputContractTemplates.generate(
            contractID: "final_feature_report_v1",
            agentID: "lead_orchestrator",
            stageID: "state_12_workflow_complete"
        ).data
        let connectUploadReceipt = try JSONSerialization.jsonObject(with: connectUploadReceiptData) as? [String: Any]
        let finalFeatureReport = try JSONSerialization.jsonObject(with: finalFeatureReportData) as? [String: Any]
        #expect((connectUploadReceipt?["status"] as? String) == "success")
        #expect((connectUploadReceipt?["artifact_id"] as? String)?.isEmpty == false)
        #expect((connectUploadReceipt?["checksum"] as? String)?.isEmpty == false)
        #expect((connectUploadReceipt?["destination"] as? String)?.isEmpty == false)
        #expect((finalFeatureReport?["final_status"] as? String) == "completed")
        #expect((finalFeatureReport?["summary"] as? String)?.isEmpty == false)
        #expect((finalFeatureReport?["cost_currency"] as? String) == "USD")
    }

    @Test("Implementation self-assessment adapter renders canonical blocked verification status")
    func implementationSelfAssessmentAdapterDerivesBlockedVerificationStatus() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let artifactURL = tempRoot.appendingPathComponent("self-assessment.json")
        try Data("""
        {
          "contract_id": "implementation_self_assessment_v2",
          "status": "blocked",
          "implementation_complete": true,
          "verification_green": false,
          "remaining_code_task_count": 0,
          "blocking_remaining_code_task_count": 0,
          "handoff_task_count": 1,
          "blocking_review_handoff_task_count": 1,
          "remaining_code_tasks": [],
          "handoff_tasks": [
            {
              "summary": "Collect signed-in smoke evidence",
              "owner_class": "manual_evidence",
              "target_stage": "release",
              "blocking_review": true,
              "evidence": "Manual evidence is outside code_writer ownership."
            }
          ],
          "known_risks": ["Verification blocked by missing toolchain"],
          "tests_run": ["cargo test: blocked"],
          "docs_impacted": [],
          "owner_class_counts": {
            "manual_evidence": 1
          },
          "target_stage_summaries": [
            {
              "target_stage": "release",
              "count": 1,
              "blocking_review_count": 1
            }
          ],
          "validation_errors": [],
          "warnings": []
        }
        """.utf8).write(to: artifactURL)

        let artifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: artifactURL.path,
            runID: UUID(),
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: [artifact]))
        #expect(summary.status == "blocked")
        #expect(summary.verificationLabel == "Blocked")
        #expect(summary.remainingCodeTasks.isEmpty)
        #expect(summary.handoffTasks.count == 1)
        #expect(summary.evidenceText.contains("cargo test: blocked"))
    }

    @Test("Implementation self-assessment adapter renders canonical invalid validation evidence")
    func implementationSelfAssessmentAdapterMarksMalformedV2PayloadsInvalid() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let artifactURL = tempRoot.appendingPathComponent("self-assessment-invalid.json")
        try Data("""
        {
          "status": "invalid",
          "implementation_complete": true,
          "verification_green": null,
          "remaining_code_tasks": [],
          "handoff_tasks": [],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": [],
          "owner_class_counts": {},
          "target_stage_summaries": [],
          "validation_errors": [
            {
              "code": "missing_required_field",
              "message": "required field verification_green is missing",
              "pointer": "$.verification_green"
            }
          ],
          "warnings": []
        }
        """.utf8).write(to: artifactURL)

        let artifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: artifactURL.path,
            runID: UUID(),
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: [artifact]))
        #expect(summary.status == "invalid")
        #expect(summary.validationErrors.contains { $0.pointer == "$.verification_green" })
        #expect(summary.evidenceText.contains("$.verification_green"))
    }

    @Test("Implementation self-assessment adapter synthesizes validation evidence for invalid task rows")
    func implementationSelfAssessmentAdapterSynthesizesValidationEvidenceForInvalidTaskRows() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let artifactURL = tempRoot.appendingPathComponent("self-assessment-invalid-task.json")
        try Data("""
        {
          "status": "invalid",
          "implementation_complete": true,
          "verification_green": true,
          "remaining_code_tasks": [
            {
              "summary": "Fix validation evidence",
              "owner": "code_writer",
              "blocking": true,
              "evidence": "   "
            }
          ],
          "handoff_tasks": [
            {
              "summary": "Route review handoff",
              "owner_class": "not_a_class",
              "target_stage": "state_9_implementation_reviewed",
              "blocking_review": true,
              "evidence": "Owner class must be normalized."
            }
          ],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": [],
          "owner_class_counts": {},
          "target_stage_summaries": [],
          "validation_errors": [
            {
              "code": "empty_required_string",
              "message": "required field evidence must not be empty",
              "pointer": "$.remaining_code_tasks[0].evidence"
            },
            {
              "code": "invalid_owner_class",
              "message": "unknown handoff owner_class: not_a_class",
              "pointer": "$.handoff_tasks[0].owner_class"
            }
          ],
          "warnings": []
        }
        """.utf8).write(to: artifactURL)

        let artifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: artifactURL.path,
            runID: UUID(),
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: [artifact]))
        #expect(summary.status == "invalid")
        #expect(summary.validationErrors.contains { $0.pointer == "$.remaining_code_tasks[0].evidence" })
        #expect(summary.validationErrors.contains { $0.pointer == "$.handoff_tasks[0].owner_class" })
    }

    @Test("Implementation self-assessment adapter renders unknown owner class as Human Triage")
    func implementationSelfAssessmentAdapterRendersUnknownOwnerClassAsHumanTriage() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let artifactURL = tempRoot.appendingPathComponent("self-assessment-unknown-owner.json")
        try Data("""
        {
          "status": "handoff_required",
          "implementation_complete": true,
          "verification_green": true,
          "remaining_code_tasks": [],
          "handoff_tasks": [
            {
              "summary": "Operator triage required",
              "owner_class": "unknown",
              "target_stage": "state_9_implementation_reviewed",
              "blocking_review": false,
              "evidence": "Ownership is not classified yet."
            }
          ],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": [],
          "owner_class_counts": {
            "unknown": 1
          },
          "target_stage_summaries": [
            {
              "target_stage": "state_9_implementation_reviewed",
              "count": 1,
              "blocking_review_count": 0
            }
          ],
          "validation_errors": [],
          "warnings": []
        }
        """.utf8).write(to: artifactURL)

        let artifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: artifactURL.path,
            runID: UUID(),
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: [artifact]))
        #expect(summary.status == "handoff_required")
        #expect(summary.handoffTasks.first?.owner == "Human Triage")
    }

    @Test("Implementation self-assessment adapter preserves canonical summary evidence fields")
    func implementationSelfAssessmentAdapterPreservesCanonicalSummaryEvidenceFields() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let artifactURL = tempRoot.appendingPathComponent("self-assessment-summary.json")
        try Data("""
        {
          "status": "invalid",
          "implementation_complete": true,
          "verification_green": true,
          "remaining_code_tasks": [
            {
              "summary": "Fix nested validation",
              "owner": "code_writer",
              "blocking": true,
              "evidence": "Missing boolean must fail closed.",
              "source_pointer": "$.remaining_code_tasks[0]"
            }
          ],
          "handoff_tasks": [],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": [],
          "owner_class_counts": {
            "release": 1
          },
          "target_stage_summaries": [
            {
              "target_stage": "state_10_release",
              "count": 1,
              "blocking_review_count": 1
            }
          ],
          "validation_errors": [
            {
              "code": "missing_bool",
              "message": "remaining_code_tasks[0].blocking must be a boolean",
              "pointer": "$.remaining_code_tasks[0].blocking"
            }
          ],
          "warnings": [
            {
              "code": "compat",
              "message": "legacy v1 fallback was ignored",
              "pointer": "$"
            }
          ]
        }
        """.utf8).write(to: artifactURL)

        let artifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: artifactURL.path,
            runID: UUID(),
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: [artifact]))
        #expect(summary.status == "invalid")
        #expect(summary.validationErrors.first?.code == "missing_bool")
        #expect(summary.warnings.first?.message == "legacy v1 fallback was ignored")
        #expect(summary.ownerClassCounts["release"] == 1)
        #expect(summary.targetStageSummaries.first?.blockingReviewCount == 1)
        #expect(summary.remainingCodeTasks.first?.sourcePointer == "$.remaining_code_tasks[0]")
        #expect(summary.evidenceText.contains("$.remaining_code_tasks[0].blocking"))
    }

    @Test("Implementation self-assessment adapter prefers embedded canonical review summary over raw artifact")
    func implementationSelfAssessmentAdapterPrefersEmbeddedCanonicalReviewSummary() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let rawArtifactURL = tempRoot.appendingPathComponent("self-assessment-raw.json")
        try Data("""
        {
          "implementation_complete": false,
          "verification_green": true,
          "remaining_code_tasks": [
            {
              "summary": "Raw file says code loop",
              "owner": "code_writer",
              "blocking": true,
              "evidence": "This raw artifact must not override the canonical summary."
            }
          ],
          "handoff_tasks": [],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": []
        }
        """.utf8).write(to: rawArtifactURL)
        let reviewSummaryURL = tempRoot.appendingPathComponent("implementation-review-summary.json")
        try Data("""
        {
          "status": "release_evidence_blocked",
          "implementation_self_assessment_summary": {
            "status": "blocked",
            "implementation_complete": true,
            "verification_green": false,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": ["Verification is blocked downstream."],
            "tests_run": ["proposal-054: blocked"],
            "docs_impacted": [],
            "owner_class_counts": {},
            "target_stage_summaries": [],
            "validation_errors": [],
            "warnings": []
          }
        }
        """.utf8).write(to: reviewSummaryURL)

        let rawArtifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: rawArtifactURL.path,
            runID: UUID(),
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )
        let reviewArtifact = Artifact(
            name: "implementation_review_summary",
            contractID: "implementation_review_summary_v1",
            format: .json,
            filePath: reviewSummaryURL.path,
            runID: rawArtifact.runID,
            stageID: "state_9_implementation_reviewed",
            agentID: "lead_orchestrator",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: [rawArtifact, reviewArtifact]))
        #expect(summary.status == "blocked")
        #expect(summary.verificationGreen == false)
        #expect(summary.remainingCodeTasks.isEmpty)
        #expect(summary.sourceArtifactName == "implementation_review_summary.implementation_self_assessment_summary")
    }

    @Test("Implementation self-assessment adapter prefers run canonical projection over raw artifact")
    func implementationSelfAssessmentAdapterPrefersRunCanonicalProjection() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
        let rawArtifactURL = tempRoot.appendingPathComponent("self-assessment-raw.json")
        try Data("""
        {
          "implementation_complete": false,
          "verification_green": true,
          "remaining_code_tasks": [
            {
              "summary": "Raw artifact wants another code pass",
              "owner": "code_writer",
              "blocking": true,
              "evidence": "The run projection must be preferred."
            }
          ],
          "handoff_tasks": [],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": []
        }
        """.utf8).write(to: rawArtifactURL)

        let run = Run(
            workflowID: "full_mvp_live",
            workflowTitle: "Full MVP",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        run.implementationSelfAssessmentSummaryJSON = Data("""
        {
          "status": "blocked",
          "implementation_complete": true,
          "verification_green": false,
          "remaining_code_tasks": [],
          "handoff_tasks": [],
          "known_risks": ["Verification is blocked downstream."],
          "tests_run": ["proposal-054: blocked"],
          "docs_impacted": [],
          "owner_class_counts": {},
          "target_stage_summaries": [],
          "validation_errors": [],
          "warnings": []
        }
        """.utf8)

        let rawArtifact = Artifact(
            name: "implementation_self_assessment",
            contractID: "implementation_self_assessment_v2",
            format: .json,
            filePath: rawArtifactURL.path,
            runID: run.id,
            stageID: "state_8_implementation_continued",
            agentID: "code_writer",
            provider: "test"
        )

        let summary = try #require(ImplementationSelfAssessmentDisplayAdapter.summary(from: run, artifacts: [rawArtifact]))
        #expect(summary.status == "blocked")
        #expect(summary.verificationGreen == false)
        #expect(summary.remainingCodeTasks.isEmpty)
        #expect(summary.sourceArtifactName == "run.implementation_self_assessment_summary")
    }

    @Test("Implementation self-assessment projection exposes transition statuses from canonical summaries")
    func implementationSelfAssessmentProjectionExposesTransitionStatusesFromCanonicalSummaries() throws {
        let cases: [(String, String)] = [
            (
                "complete",
                """
                {
                  "status": "complete",
                  "implementation_complete": true,
                  "verification_green": true,
                  "remaining_code_tasks": [],
                  "handoff_tasks": [],
                  "known_risks": [],
                  "tests_run": ["proposal-054: pass"],
                  "docs_impacted": [],
                  "owner_class_counts": {},
                  "target_stage_summaries": [],
                  "validation_errors": [],
                  "warnings": []
                }
                """
            ),
            (
                "needs_code_fixes",
                """
                {
                  "status": "needs_code_fixes",
                  "implementation_complete": false,
                  "verification_green": true,
                  "remaining_code_tasks": [],
                  "handoff_tasks": [],
                  "known_risks": [],
                  "tests_run": ["proposal-054: failing"],
                  "docs_impacted": [],
                  "owner_class_counts": {},
                  "target_stage_summaries": [],
                  "validation_errors": [],
                  "warnings": []
                }
                """
            ),
            (
                "blocked",
                """
                {
                  "status": "blocked",
                  "implementation_complete": true,
                  "verification_green": false,
                  "remaining_code_tasks": [],
                  "handoff_tasks": [],
                  "known_risks": ["xcodebuild unavailable"],
                  "tests_run": ["proposal-054: blocked"],
                  "docs_impacted": [],
                  "owner_class_counts": {},
                  "target_stage_summaries": [],
                  "validation_errors": [],
                  "warnings": []
                }
                """
            ),
            (
                "handoff_required",
                """
                {
                  "status": "handoff_required",
                  "implementation_complete": true,
                  "verification_green": true,
                  "remaining_code_tasks": [],
                  "handoff_tasks": [
                    {
                      "summary": "Capture signed-in smoke evidence",
                      "owner_class": "manual_evidence",
                      "target_stage": "state_9_implementation_reviewed",
                      "blocking_review": true,
                      "evidence": "Manual smoke evidence remains outside code_writer."
                    }
                  ],
                  "known_risks": [],
                  "tests_run": ["proposal-054: pass"],
                  "docs_impacted": [],
                  "owner_class_counts": {
                    "manual_evidence": 1
                  },
                  "target_stage_summaries": [
                    {
                      "target_stage": "state_9_implementation_reviewed",
                      "count": 1,
                      "blocking_review_count": 1
                    }
                  ],
                  "validation_errors": [],
                  "warnings": []
                }
                """
            ),
            (
                "invalid",
                """
                {
                  "status": "invalid",
                  "implementation_complete": true,
                  "verification_green": null,
                  "remaining_code_tasks": [],
                  "handoff_tasks": [],
                  "known_risks": [],
                  "tests_run": [],
                  "docs_impacted": [],
                  "owner_class_counts": {},
                  "target_stage_summaries": [],
                  "validation_errors": [
                    {
                      "code": "missing_required_field",
                      "message": "required field verification_green is missing",
                      "pointer": "/verification_green"
                    }
                  ],
                  "warnings": []
                }
                """
            ),
            (
                "unknown",
                """
                {
                  "status": "unknown",
                  "implementation_complete": true,
                  "verification_green": null,
                  "remaining_code_tasks": [],
                  "handoff_tasks": [],
                  "known_risks": [],
                  "tests_run": [],
                  "docs_impacted": [],
                  "owner_class_counts": {},
                  "target_stage_summaries": [],
                  "validation_errors": [],
                  "warnings": [
                    {
                      "code": "legacy_v1_compatibility",
                      "message": "legacy implementation_self_assessment was mapped by the canonical domain parser",
                      "pointer": "/seemingly_complete"
                    }
                  ]
                }
                """
            ),
        ]

        for (expectedStatus, payload) in cases {
            let canonicalData = try #require(
                ImplementationSelfAssessmentSummaryProjection.canonicalSummaryData(
                    from: Data(payload.utf8),
                    artifactName: "implementation_self_assessment"
                )
            )
            let object = try #require(
                JSONSerialization.jsonObject(with: canonicalData) as? [String: Any]
            )
            #expect(object["status"] as? String == expectedStatus)

            let fields = try #require(
                ImplementationSelfAssessmentSummaryProjection.scalarFields(
                    fromCanonicalSummaryData: canonicalData
                )
            )
            guard case .string(let projectedStatus) = fields["status"] else {
                Issue.record("status scalar was not projected for \(expectedStatus)")
                continue
            }
            #expect(projectedStatus == expectedStatus)
        }
    }

    @Test("Implementation self-assessment projection ignores raw v2 artifacts")
    func implementationSelfAssessmentProjectionIgnoresRawV2Artifacts() {
        let payload = """
        {
          "implementation_complete": true,
          "verification_green": true,
          "remaining_code_tasks": [],
          "handoff_tasks": [],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": []
        }
        """

        let canonicalData = ImplementationSelfAssessmentSummaryProjection.canonicalSummaryData(
            from: Data(payload.utf8),
            artifactName: "implementation_self_assessment"
        )
        #expect(canonicalData == nil)
    }

    @Test("Implementation self-assessment strict validation rejects malformed nested task fields")
    func implementationSelfAssessmentStrictValidationRejectsMalformedNestedTaskFields() {
        let catalog = makeP054TestCatalog()
        let agent = makeP054CodeWriterAgent()
        let data = Data("""
        {
          "implementation_complete": true,
          "verification_green": true,
          "remaining_code_tasks": [
            {
              "summary": "Missing blocking",
              "owner": "code_writer",
              "evidence": "The blocking field must be explicit."
            }
          ],
          "handoff_tasks": [],
          "known_risks": [],
          "tests_run": [],
          "docs_impacted": []
        }
        """.utf8)

        let results = OutputContractResolverV2.validateOutputs(
            ["implementation_self_assessment": data],
            agent: agent,
            catalog: catalog
        )

        let result = results["implementation_self_assessment"]
        #expect(result?.status == .failed)
        #expect(result?.validationError?.contains("remaining_code_tasks[0].blocking") == true)
    }

    @Test("Portability-sensitive runtime sources avoid workstation-specific absolute paths")
    func portabilitySensitiveSourcesAvoidHardcodedUserPaths() throws {
        let repoRoot = testRepositoryRootURL()
        let sensitiveFiles = [
            "Chainworks Forge/Support/PreviewSupport.swift",
            "Chainworks Forge/Views/DeliveryPreflightReportView.swift",
            "Chainworks Forge/Views/ReleaseGateView.swift",
            "Chainworks Forge/Views/IdeaListView.swift",
            "Chainworks ForgeTests/Chainworks_ForgeTests.swift",
            "Chainworks ForgeTests/RuntimeSessionBridgeTests.swift"
        ]

        for relativePath in sensitiveFiles {
            let fileURL = repoRoot.appendingPathComponent(relativePath, isDirectory: false)
            guard let source = try? String(contentsOf: fileURL, encoding: .utf8) else {
                // Source tree not accessible from sandboxed test process — guardrail
                // in test-gate.sh covers this check via guard_portability_paths.
                continue
            }
            #expect(
                source.contains("/Users/user/") == false,
                "\(relativePath) still hardcodes a workstation-specific user path"
            )
        }
    }

    @Test("Preferred example URL resolves repo copy before bundled fallback")
    func preferredExampleURLPrefersRepositoryCopy() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: tempRoot.appendingPathComponent("examples/agents", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        let repoCopy = tempRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false)
        let bundledCopy = tempRoot.appendingPathComponent("agents.bundle.yaml", isDirectory: false)
        try "repo".write(to: repoCopy, atomically: true, encoding: .utf8)
        try "bundle".write(to: bundledCopy, atomically: true, encoding: .utf8)

        // sourceFilePath must resolve so that repositoryRootDerivedFromSourcePath
        // walks up 3 levels to tempRoot — matching currentDirectoryPath and passing
        // the SecurityScopedAccess guard on currentDirectoryURL.
        let syntheticSourceFile = tempRoot
            .appendingPathComponent("Chainworks Forge/Support/Fake.swift").path

        let resolved = AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/agents/agents.yaml",
            bundledURL: bundledCopy,
            currentDirectoryPath: tempRoot.path,
            allowsDocumentsFallback: false,
            sourceFilePath: syntheticSourceFile
        )

        #expect(resolved?.path == repoCopy.path)
    }

    @Test("Preferred example URL can anchor repository lookup to caller source file")
    func preferredExampleURLUsesCallerSourceFilePath() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceRoot = tempRoot.appendingPathComponent("repo", isDirectory: true)
        let sourceFile = sourceRoot
            .appendingPathComponent("Chainworks Forge/Engine/Proposal022FeedbackFidelity.swift", isDirectory: false)
        let bundledCopy = tempRoot.appendingPathComponent("proposal-loop-live.bundle.yaml", isDirectory: false)

        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("examples/workflows", isDirectory: true),
            withIntermediateDirectories: true
        )
        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("Chainworks Forge/Engine", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        let repoCopy = sourceRoot.appendingPathComponent("examples/workflows/proposal-loop-live.yaml", isDirectory: false)
        try "repo".write(to: repoCopy, atomically: true, encoding: .utf8)
        try "source".write(to: sourceFile, atomically: true, encoding: .utf8)
        try "bundle".write(to: bundledCopy, atomically: true, encoding: .utf8)

        let resolved = AppConfiguration.preferredExampleURL(
            repoRelativePath: "examples/workflows/proposal-loop-live.yaml",
            bundledURL: bundledCopy,
            currentDirectoryPath: tempRoot.appendingPathComponent("elsewhere", isDirectory: true).path,
            allowsDocumentsFallback: false,
            sourceFilePath: sourceFile.path
        )

        #expect(resolved?.path == repoCopy.path)
    }

    @Test("Repository root resolution prefers source checkout over Documents fallback")
    func defaultRepositoryRootPrefersSourceCheckout() throws {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sourceRoot = tempRoot.appendingPathComponent("repo", isDirectory: true)
        let sourceFile = sourceRoot
            .appendingPathComponent("Chainworks Forge/Support/AppConfiguration.swift", isDirectory: false)
        let documentsRoot = tempRoot.appendingPathComponent("Documents/Chainworks Forge", isDirectory: true)

        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("examples/agents", isDirectory: true),
            withIntermediateDirectories: true
        )
        try FileManager.default.createDirectory(
            at: sourceRoot.appendingPathComponent("Chainworks Forge/Support", isDirectory: true),
            withIntermediateDirectories: true
        )
        try FileManager.default.createDirectory(
            at: documentsRoot.appendingPathComponent("examples/agents", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: tempRoot) }

        try "repo".write(
            to: sourceRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false),
            atomically: true,
            encoding: .utf8
        )
        try "source".write(to: sourceFile, atomically: true, encoding: .utf8)
        try "documents".write(
            to: documentsRoot.appendingPathComponent("examples/agents/agents.yaml", isDirectory: false),
            atomically: true,
            encoding: .utf8
        )

        let resolved = AppConfiguration.defaultRepositoryRoot(
            currentDirectoryPath: tempRoot.appendingPathComponent("elsewhere", isDirectory: true).path,
            bundleURL: nil,
            allowsDocumentsFallback: true,
            sourceFilePath: sourceFile.path
        )

        #expect(resolved.standardizedFileURL == sourceRoot.standardizedFileURL)
    }

    @Test("Repo-backed seed surfaces avoid cwd-derived repository roots")
    func seededRuntimeSurfacesAvoidCurrentDirectoryRepoTruth() throws {
        let repoRoot = testRepositoryRootURL()
        let sensitiveFiles = [
            "Chainworks Forge/Chainworks_ForgeApp.swift",
            "Chainworks Forge/Engine/SampleRunLauncher.swift"
        ]
        let forbiddenFragments = [
            "repoRoot: FileManager.default.currentDirectoryPath",
            "run.repoRoot = FileManager.default.currentDirectoryPath",
            "workspaceRootPath: FileManager.default.currentDirectoryPath"
        ]

        for relativePath in sensitiveFiles {
            let fileURL = repoRoot.appendingPathComponent(relativePath, isDirectory: false)
            guard let source = try? String(contentsOf: fileURL, encoding: .utf8) else {
                // Source tree not accessible — guardrail covers this.
                continue
            }
            for fragment in forbiddenFragments {
                #expect(
                    source.contains(fragment) == false,
                    "\(relativePath) still derives repo-backed runtime truth from cwd via: \(fragment)"
                )
            }
        }
    }

    @Test("Run report payload exposes requested predicted actual MCP contract and telemetry")
    func runReportPayloadExposesMCPTruth() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)

        let stage = StageExecution(
            stageID: "state_4_proposal_reviewed",
            label: "Proposal Reviewed",
            status: .completed
        )
        stage.run = run
        context.insert(stage)

        let agent = AgentExecution(
            agentID: "proposal_reviewer_ui",
            agentTitle: "UI Reviewer",
            taskName: "review_ui",
            status: .completed,
            provider: "gemini",
            effort: "high"
        )
        agent.stageExecution = stage
        agent.mcpProfileID = "ui_review_visual"
        agent.requestedMCPExtensionsJSON = try JSONEncoder().encode(["xcode", "context7"] as [String])
        agent.effectiveMCPRuntimeExtensionIDsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agent.deniedMCPExtensionsJSON = try JSONEncoder().encode(["context7"] as [String])
        agent.mcpSessionStartupLatencyMilliseconds = 240
        agent.mcpServerTelemetryJSON = try JSONEncoder().encode([
            MCPServerExecutionMetric(
                serverID: "xcode",
                toolCallCount: 2,
                requestBytes: 128,
                responseBytes: 512,
                promptContextDeltaBytes: 512
            )
        ])
        context.insert(agent)

        let resolvedPolicies: [String: MCPPolicyResolutionReport] = [
            "proposal_reviewer_ui": MCPPolicyResolutionReport(
                profileID: "ui_review_visual",
                requiredExtensions: ["xcode"],
                optionalExtensions: ["context7"],
                requestedExtensions: ["xcode", "context7"],
                requiredRuntimeExtensionIDs: ["xcode"],
                optionalRuntimeExtensionIDs: ["context7"],
                predictedEffectiveExtensions: ["xcode", "context7"],
                predictedEffectiveRuntimeExtensionIDs: ["xcode", "context7"],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: []
            )
        ]
        run.resolvedMCPPoliciesJSON = try JSONEncoder().encode(resolvedPolicies)
        run.sessionKPIExportJSON = SessionReuseKPIExporter.exportJSON(for: run.id, context: context)

        let payload = RunReportBuilder(modelContext: context).buildReportPayload(for: run, version: 1)
        let reportAgent = try #require(payload.agentsUsed.first(where: { $0.agentID == "proposal_reviewer_ui" }))
        let mcpTelemetry = try #require(payload.mcpTelemetry)

        #expect(reportAgent.mcpProfileID == "ui_review_visual")
        #expect(reportAgent.requestedMCPExtensions == ["xcode", "context7"])
        #expect(reportAgent.predictedMCPExtensions == ["xcode", "context7"])
        #expect(reportAgent.actualMCPExtensions == ["xcode"])
        #expect(reportAgent.deniedMCPExtensions == ["context7"])
        #expect(mcpTelemetry.totalExecutionsWithMCPProfile == 1)
        #expect(mcpTelemetry.totalRequestedExtensionCount == 2)
        #expect(mcpTelemetry.totalActualExtensionCount == 1)
        #expect(mcpTelemetry.totalDeniedExtensionCount == 1)
        #expect(mcpTelemetry.totalPolicyReductionExecutions == 1)
        #expect(mcpTelemetry.totalPredictionDriftExecutions == 1)
        #expect(mcpTelemetry.totalStartupLatencyMilliseconds == 240)
        #expect(mcpTelemetry.averageStartupLatencyMilliseconds == 240)
        #expect(mcpTelemetry.totalPromptContextDeltaBytes == 512)
        #expect(mcpTelemetry.totalMCPPreflightBlockedRuns == 0)
        let latencyBucket = try #require(mcpTelemetry.startupLatencyByExtensionSet.first)
        #expect(latencyBucket.extensionSet == "xcode")
        #expect(latencyBucket.executionCount == 1)
        let usage = try #require(mcpTelemetry.serverUsage.first(where: { $0.serverID == "xcode" }))
        #expect(usage.toolCallCount == 2)
        #expect(usage.requestBytes == 128)
        #expect(usage.responseBytes == 512)
        #expect(usage.promptContextDeltaBytes == 512)
    }

    @Test("KPI summary counts MCP-preflight blocked runs")
    func mcpPreflightBlockedRunsAreCounted() throws {
        let (_, context) = try makeTestModelContainer()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        run.status = .blocked
        run.driftDetails = "Extension registry is unavailable, but one or more agents request MCP extensions."

        let summary = SessionReuseKPIExporter.exportKPIs(for: run.id, context: context)
        #expect(summary.mcpTelemetry.totalMCPPreflightBlockedRuns == 1)
    }

    @Test("Run comparison surfaces MCP contract deltas for each agent binding")
    func runComparisonSurfacesMCPContract() throws {
        let (_, context) = try makeTestModelContainer()
        let idea = Idea(title: "Proposal 025", body: "Compare MCP truth")
        context.insert(idea)

        let runA = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        let runB = Run(
            workflowID: "proposal_loop_live",
            workflowTitle: "Proposal Loop",
            workflowSnapshotHash: "wf",
            catalogSnapshotHash: "cat",
            workflowSourcePath: "workflow.yaml",
            catalogSourcePath: "agents.yaml",
            workflowSnapshotJSON: Data("{}".utf8),
            catalogSnapshotJSON: Data("{}".utf8)
        )
        runA.idea = idea
        runB.idea = idea
        context.insert(runA)
        context.insert(runB)

        let stageA = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal Reviewed", status: .completed)
        stageA.run = runA
        context.insert(stageA)
        let stageB = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal Reviewed", status: .completed)
        stageB.run = runB
        context.insert(stageB)

        let agentA = AgentExecution(agentID: "proposal_reviewer_ui", agentTitle: "UI Reviewer", taskName: "review_ui", status: .completed, provider: "gemini", effort: "high")
        agentA.stageExecution = stageA
        agentA.mcpProfileID = "ui_review_visual"
        agentA.requestedMCPExtensionsJSON = try JSONEncoder().encode(["xcode", "context7"] as [String])
        agentA.effectiveMCPRuntimeExtensionIDsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agentA.deniedMCPExtensionsJSON = try JSONEncoder().encode(["context7"] as [String])
        context.insert(agentA)

        let agentB = AgentExecution(agentID: "proposal_reviewer_ui", agentTitle: "UI Reviewer", taskName: "review_ui", status: .completed, provider: "gemini", effort: "high")
        agentB.stageExecution = stageB
        agentB.mcpProfileID = "ui_review_visual"
        agentB.requestedMCPExtensionsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agentB.effectiveMCPRuntimeExtensionIDsJSON = try JSONEncoder().encode(["xcode"] as [String])
        agentB.deniedMCPExtensionsJSON = try JSONEncoder().encode([String]())
        context.insert(agentB)

        let policiesA = [
            "proposal_reviewer_ui": MCPPolicyResolutionReport(
                profileID: "ui_review_visual",
                requiredExtensions: ["xcode"],
                optionalExtensions: ["context7"],
                requestedExtensions: ["xcode", "context7"],
                requiredRuntimeExtensionIDs: ["xcode"],
                optionalRuntimeExtensionIDs: ["context7"],
                predictedEffectiveExtensions: ["xcode", "context7"],
                predictedEffectiveRuntimeExtensionIDs: ["xcode", "context7"],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: []
            )
        ]
        let policiesB = [
            "proposal_reviewer_ui": MCPPolicyResolutionReport(
                profileID: "ui_review_visual",
                requiredExtensions: ["xcode"],
                optionalExtensions: [],
                requestedExtensions: ["xcode"],
                requiredRuntimeExtensionIDs: ["xcode"],
                optionalRuntimeExtensionIDs: [],
                predictedEffectiveExtensions: ["xcode"],
                predictedEffectiveRuntimeExtensionIDs: ["xcode"],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: []
            )
        ]
        runA.resolvedMCPPoliciesJSON = try JSONEncoder().encode(policiesA)
        runB.resolvedMCPPoliciesJSON = try JSONEncoder().encode(policiesB)

        let comparison = try #require(RunComparisonService(modelContext: context).compare(runA, runB))
        let bindingA = try #require(comparison.bindingsA.first(where: { $0.agentID == "proposal_reviewer_ui" }))
        let bindingB = try #require(comparison.bindingsB.first(where: { $0.agentID == "proposal_reviewer_ui" }))

        #expect(bindingA.requestedMCPExtensions == ["xcode", "context7"])
        #expect(bindingA.predictedMCPExtensions == ["xcode", "context7"])
        #expect(bindingA.actualMCPExtensions == ["xcode"])
        #expect(bindingA.deniedMCPExtensions == ["context7"])
        #expect(bindingB.requestedMCPExtensions == ["xcode"])
        #expect(bindingB.predictedMCPExtensions == ["xcode"])
        #expect(bindingB.actualMCPExtensions == ["xcode"])
        #expect(bindingB.deniedMCPExtensions.isEmpty)
    }
}

private func makeP054TestCatalog() -> AgentCatalog {
    AgentCatalog(
        schemaVersion: 1,
        app: AppConfig(
            name: "test",
            runtime: "claude_agent",
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
        contracts: [
            "implementation_self_assessment_v2": ArtifactContract(
                format: "json",
                requiredFields: [
                    "implementation_complete",
                    "verification_green",
                    "remaining_code_tasks",
                    "handoff_tasks",
                    "known_risks",
                    "tests_run",
                    "docs_impacted"
                ]
            )
        ],
        backendProfiles: [
            "test_profile": BackendProfile(
                provider: "codex",
                model: "gpt-5.4",
                effort: "high",
                temperature: 0.0,
                maxTurns: 20,
                structuredOutput: "required"
            )
        ],
        permissionProfiles: [:],
        runtimeProfiles: [:],
        agents: []
    )
}

private func makeP054CodeWriterAgent() -> ResolvedAgent {
    ResolvedAgent(
        id: "code_writer",
        title: "Code Writer",
        mode: "implementation",
        backendProfileID: "test_profile",
        provider: "codex",
        model: "gpt-5.4",
        effort: "high",
        maxTurns: 18,
        temperature: 0.0,
        permissionProfile: "WRITE",
        skillRef: "code_writer_core",
        skillRole: nil,
        prompt: "Implement the proposal",
        outputContract: "implementation_self_assessment_v2",
        requiresHumanApproval: false,
        inputs: [],
        outputs: ["implementation_self_assessment"],
        worktreeWriteEnabled: true
    )
}
