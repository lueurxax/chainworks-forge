import Foundation

// MARK: - Proposal 013 Layer O: Artifact Persistence Ordering Policy

/// Freezes the order of raw-output persistence, receipt persistence,
/// structured validation, and stage settlement.
///
/// Required ordering (§6.2):
/// 1. AgentExecutor returns raw payloads, receipt, transcript, timing
/// 2. ArtifactManager persists raw + receipt + transcript (provisional metadata)
/// 3. WorkflowOrchestrator validates structured outputs via OutputContractResolverV2
/// 4. ArtifactManager persists normalized artifacts or ValidationFailureRecord
/// 5. WorkflowOrchestrator settles AgentExecution, StageExecution, Run status
/// 6. RunReportBuilder materializes immutable report truth
///
/// This policy ensures validation never destroys evidence.
enum ArtifactPersistenceOrderingPolicy {

    // MARK: - Step 1: Persist Raw Outputs (before validation)

    /// Persist raw agent outputs to disk before any validation.
    /// Returns structured output envelopes for tracking.
    @MainActor
    static func persistRawOutputs(
        result: AgentResult,
        agent: ResolvedAgent,
        agentExecution: AgentExecution,
        workspace: RunWorkspace,
        stageID: String,
        iteration: Int,
        attemptNumber: Int,
        artifactManager: ArtifactManager,
        catalog: AgentCatalog?
    ) throws -> (artifacts: [Artifact], envelopes: [StructuredOutputEnvelope]) {
        var envelopes: [StructuredOutputEnvelope] = []

        // Proposal 013 §8.2: Apply compaction to proposal drafting outputs before persistence
        var compactedOutputs = result.outputs
        for (name, data) in compactedOutputs {
            if name.hasPrefix("proposal_") {
                let compactionResult = ProposalDraftCompactionPolicy.apply(outputName: name, data: data)
                if compactionResult.wasCompacted {
                    compactedOutputs[name] = compactionResult.data
                    // Persist compaction metadata on the agent execution
                    if let metadata = compactionResult.metadata {
                        agentExecution.compactionMetadataJSON = try? JSONEncoder().encode(metadata)
                    }
                }
            }
        }

        // Persist all raw outputs via ArtifactManager (ARCH-030)
        let artifacts = try artifactManager.persistOutputs(
            outputs: compactedOutputs,
            agent: agent,
            agentExecution: agentExecution,
            workspace: workspace,
            stageID: stageID,
            iteration: iteration,
            attemptNumber: attemptNumber,
            catalog: catalog
        )

        // Build envelopes for tracking
        for (name, data) in result.outputs {
            let contractID = OutputContractResolverV2.resolveContractID(
                for: name,
                agent: agent,
                catalog: catalog
            )
            let checksum = artifacts.first(where: { $0.name == name })?.checksumSHA256

            let envelope = StructuredOutputEnvelope(
                outputName: name,
                agentID: agent.id,
                stageID: stageID,
                runID: workspace.runID,
                rawPayloadSize: data.count,
                rawPayloadChecksum: checksum,
                rawPayloadPersisted: true,
                contractID: contractID,
                provider: agent.provider,
                model: agent.model,
                effort: agent.effort,
                sessionID: result.sessionID,
                durationSeconds: result.durationSeconds
            )
            envelopes.append(envelope)
        }

        return (artifacts, envelopes)
    }

    // MARK: - Step 2: Validate Outputs (after raw persistence)

    /// Run structured validation against persisted raw outputs.
    /// Returns validation results and updated envelopes.
    static func validatePersistedOutputs(
        outputs: [String: Data],
        agent: ResolvedAgent,
        catalog: AgentCatalog?,
        envelopes: inout [StructuredOutputEnvelope]
    ) -> [String: OutputValidationResult] {
        // Proposal 013 §4.4: Use ProposalReviewContractAdapter for review outputs
        var results: [String: OutputValidationResult] = [:]
        for (name, data) in outputs {
            if ProposalReviewContractAdapter.isReviewOutput(name) || ProposalReviewContractAdapter.isReviewSummary(name) {
                results[name] = ProposalReviewContractAdapter.validateReviewOutput(
                    outputName: name,
                    data: data,
                    catalog: catalog
                )
            }
        }
        // Non-review outputs: use generic V2 validation
        let nonReviewOutputs = outputs.filter { !ProposalReviewContractAdapter.isReviewOutput($0.key) && !ProposalReviewContractAdapter.isReviewSummary($0.key) }
        if !nonReviewOutputs.isEmpty {
            let v2Results = OutputContractResolverV2.validateOutputs(
                nonReviewOutputs,
                agent: agent,
                catalog: catalog
            )
            results.merge(v2Results) { existing, _ in existing }
        }

        // Update envelopes with validation results
        for i in envelopes.indices {
            if let result = results[envelopes[i].outputName] {
                // Create updated envelope with validation result
                let old = envelopes[i]
                envelopes[i] = StructuredOutputEnvelope(
                    id: old.id,
                    timestamp: old.timestamp,
                    outputName: old.outputName,
                    agentID: old.agentID,
                    stageID: old.stageID,
                    runID: old.runID,
                    rawPayloadSize: old.rawPayloadSize,
                    rawPayloadChecksum: old.rawPayloadChecksum,
                    rawPayloadPersisted: old.rawPayloadPersisted,
                    contractID: old.contractID,
                    validationResult: result,
                    normalizedArtifactProduced: result.status == .passed,
                    provider: old.provider,
                    model: old.model,
                    effort: old.effort,
                    sessionID: old.sessionID,
                    durationSeconds: old.durationSeconds
                )
            }
        }

        return results
    }

    // MARK: - Step 3: Build Failure Record (if validation failed)

    /// Build a ValidationFailureRecord from validation results.
    /// Returns nil if all validations passed.
    static func buildFailureRecord(
        validationResults: [String: OutputValidationResult],
        agent: ResolvedAgent,
        stageID: String,
        runID: UUID,
        rawOutputExists: Bool,
        receiptExists: Bool,
        transcriptExists: Bool,
        catalog: AgentCatalog?
    ) -> ValidationFailureRecord? {
        let failedResults = validationResults.values.filter { $0.status == .failed }
        guard !failedResults.isEmpty else { return nil }

        let failureSummary = failedResults
            .map { result in
                let explanation = result.validationError ?? "Validation failed"
                return "\(result.outputName): \(explanation)"
            }
            .joined(separator: "; ")

        let contractMetadata = failedResults.compactMap { result -> ContractValidationMetadata? in
            guard let contractID = result.contractID,
                  let schema = OutputContractResolverV2.resolveSchema(for: result.outputName, agent: agent, catalog: catalog) else {
                return nil
            }
            return ContractValidationMetadata(
                outputName: result.outputName,
                contractID: contractID,
                machineFormat: schema.machineFormat.rawValue,
                validationMode: schema.validationMode.rawValue,
                requiredFieldCount: schema.requiredFields.count,
                rawArtifactName: schema.rawArtifactName,
                normalizedArtifactName: schema.normalizedArtifactName
            )
        }

        let recommendation = RecoveryRecommendation(
            action: .retryFailedAgent,
            explanation: "Output contract mismatch — raw outputs exist, retry the agent with the same inputs.",
            source: .runtimePolicy
        )

        return ValidationFailureRecord(
            agentID: agent.id,
            stageID: stageID,
            runID: runID,
            outputResults: Array(validationResults.values),
            failureSummary: failureSummary.isEmpty ? "Output validation failed" : failureSummary,
            failureClass: .outputContractMismatch,
            contractMetadata: contractMetadata,
            rawOutputExists: rawOutputExists,
            receiptExists: receiptExists,
            transcriptExists: transcriptExists,
            recoveryRecommendation: recommendation
        )
    }

    // MARK: - Step 4: Persist Failure Evidence

    /// Persist the validation failure record as an artifact.
    @MainActor
    static func persistFailureEvidence(
        failureRecord: ValidationFailureRecord,
        workspace: RunWorkspace,
        stageID: String,
        agentID: String,
        attemptNumber: Int,
        artifactManager: ArtifactManager
    ) throws -> Artifact {
        let data = try JSONEncoder().encode(failureRecord)

        return try artifactManager.persistSystemArtifact(
            name: "validation_failure_\(agentID)",
            data: data,
            contractID: "validation_failure_record",
            format: .json,
            workspace: workspace,
            stageID: stageID,
            agentID: agentID,
            provider: "system",
            model: nil,
            effort: nil,
            attemptNumber: attemptNumber
        )
    }

    // MARK: - Agent Retry Namespace (§5.4)

    /// Compute the artifact path namespace for an agent-only retry.
    /// Per §5.4: agent-retry artifacts live in a disjoint namespace:
    /// {artifactRoot}/{stageID}.{iteration}/{agentID}/{stageAttemptNumber}/agent-retry-{agentAttemptNumber}/{name}
    static func agentRetryNamespace(
        stageID: String,
        iteration: Int,
        agentID: String,
        stageAttemptNumber: Int,
        agentAttemptNumber: Int
    ) -> String {
        "\(stageID).\(iteration)/\(agentID)/\(stageAttemptNumber)/agent-retry-\(agentAttemptNumber)"
    }
}
