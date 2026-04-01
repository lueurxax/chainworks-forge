import Foundation

struct AgentSessionCheckpoint: Codable, Sendable {
    let machineSummary: String
    let nextSteps: [String]
    let durableLearnings: [String]
    let unresolvedBlockers: [String]
    let openDecisions: [String]
    let openQuestions: [String]
    let unresolvedConstraints: [String]
    let selectedArtifactReferences: [UUID]
    let lastValidatedAggregateStateJSON: Data?
    let ownerAndBindingContextJSON: Data?
    let scopeContextJSON: Data?
    let compactedConversationStateJSON: Data?
}

final class AgentSessionCheckpointBuilder {

    /// Build a checkpoint from a completed execution result and event log (§6.4).
    ///
    /// Produces a continuation artifact that preserves enough explicit state for
    /// a fresh generation to continue deterministically without opaque provider memory.
    static func build(
        executionResult: AgentResult,
        eventLog: [ExecutionEvent],
        ownerKey: String? = nil,
        scope: SessionReuseScope? = nil,
        familyID: String? = nil,
        artifactReferences: [UUID] = [],
        validatedAggregateState: Data? = nil
    ) -> AgentSessionCheckpoint {
        let context = buildOwnerContext(ownerKey: ownerKey, scope: scope, familyID: familyID)
        let contextData = try? JSONEncoder().encode(context)
        let scopeData = buildScopeContextData(scope: scope, familyID: familyID)

        // Scan event log for blockers, decisions, and questions (continuation-safe extraction)
        var blockers: [String] = []
        var questions: [String] = []
        var decisions: [String] = []
        var constraints: [String] = []
        var toolCallSummaries: [String] = []

        for event in eventLog {
            switch event.type {
            case .textChunk:
                if event.detail.contains("BLOCKER:") { blockers.append(event.detail) }
                if event.detail.contains("QUESTION:") { questions.append(event.detail) }
                if event.detail.contains("DECISION:") { decisions.append(event.detail) }
                if event.detail.contains("CONSTRAINT:") { constraints.append(event.detail) }
            case .toolCallFinished:
                toolCallSummaries.append("Tool: \(event.detail)")
            default:
                break
            }
        }

        let outcome = executionResult.canonicalOutcome?.rawValue ?? "unknown"
        let artifactNames = executionResult.outputs.keys.sorted().joined(separator: ", ")
        let summary = "Execution completed with outcome: \(outcome). " +
            "Duration: \(String(format: "%.1f", executionResult.durationSeconds))s. " +
            "Session: \(executionResult.sessionID ?? "unknown"). " +
            "Artifacts: \(artifactNames.isEmpty ? "none" : artifactNames)"

        // Determine next steps based on outcome
        var nextSteps: [String]
        switch executionResult.canonicalOutcome {
        case .completed:
            nextSteps = ["Continue with next planned stage."]
        case .failedBeforeOutput:
            nextSteps = ["Retry execution; no outputs were produced."]
        case .completedWithTransportError:
            nextSteps = ["Verify partial outputs, then retry or continue."]
        case .limitExhaustedAfterOutput, .limitExhaustedBeforeOutput:
            nextSteps = ["Budget exhausted; review limits before retry."]
        default:
            nextSteps = ["Assess failure cause before proceeding."]
        }

        // Durable learnings: what we know from this execution
        var learnings: [String] = []
        if !artifactNames.isEmpty {
            learnings.append("Generated artifacts: \(artifactNames)")
        }
        if !toolCallSummaries.isEmpty {
            learnings.append("Tool calls executed: \(toolCallSummaries.count)")
        }
        if let provider = executionResult.runtimeProvider, let model = executionResult.runtimeModel {
            learnings.append("Provider: \(provider)/\(model)")
        }

        // Build validated aggregate state: use caller-provided payload, or construct from outputs.
        // Per §6.4: "last validated aggregate state" should be a real state payload,
        // not just a list of output names.
        let aggregateState: Data? = if let provided = validatedAggregateState {
            provided
        } else if !executionResult.outputs.isEmpty {
            // Build a structured aggregate from actual output data sizes and names
            try? JSONEncoder().encode(
                executionResult.outputs.map { (key, value) in
                    ["name": key, "sizeBytes": "\(value.count)", "present": "true"]
                }
            )
        } else {
            nil
        }

        // Build compacted conversation state from event log summary
        let compactedState: Data? = if !eventLog.isEmpty {
            try? JSONEncoder().encode([
                "eventCount": "\(eventLog.count)",
                "toolCalls": "\(toolCallSummaries.count)",
                "hasBlockers": "\(!blockers.isEmpty)",
                "hasQuestions": "\(!questions.isEmpty)",
                "hasDecisions": "\(!decisions.isEmpty)",
                "hasConstraints": "\(!constraints.isEmpty)"
            ])
        } else {
            nil
        }

        return AgentSessionCheckpoint(
            machineSummary: summary,
            nextSteps: nextSteps,
            durableLearnings: learnings,
            unresolvedBlockers: blockers,
            openDecisions: decisions,
            openQuestions: questions,
            unresolvedConstraints: constraints,
            selectedArtifactReferences: artifactReferences,
            lastValidatedAggregateStateJSON: aggregateState,
            ownerAndBindingContextJSON: contextData,
            scopeContextJSON: scopeData,
            compactedConversationStateJSON: compactedState
        )
    }

    /// Build a checkpoint specifically for operator-reset (§6.4 rule 1).
    ///
    /// Emitted before the session is retired so that the next fresh generation
    /// can rehydrate from durable state rather than relying on opaque provider memory.
    static func buildForReset(
        generation: AgentSessionGeneration,
        lineage: AgentSessionLineage,
        resetReason: String
    ) -> AgentSessionCheckpoint {
        let context = buildOwnerContext(
            ownerKey: generation.invocationOwnerKey,
            scope: lineage.sessionReuseScope,
            familyID: lineage.sessionFamilyID
        )
        let contextData = try? JSONEncoder().encode(context)
        let scopeData = buildScopeContextData(scope: lineage.sessionReuseScope, familyID: lineage.sessionFamilyID)

        let summary = "Session reset (generation #\(generation.generation)). " +
            "Turns: \(generation.turnCount), " +
            "Cumulative tokens: \(generation.cumulativePromptTokens), " +
            "Cumulative cost: \(generation.cumulativeCostCents)c. " +
            "Reason: \(resetReason)"

        return AgentSessionCheckpoint(
            machineSummary: summary,
            nextSteps: ["Fresh session will be created on next invocation."],
            durableLearnings: [
                "Generation #\(generation.generation) processed \(generation.turnCount) turns.",
                "Provider: \(generation.runtimeProvider)/\(generation.runtimeModel).",
                "Working directory: \(generation.workingDirectory) (\(generation.workspaceMode))."
            ],
            unresolvedBlockers: [],
            openDecisions: ["Operator chose to reset; next invocation starts fresh."],
            openQuestions: [],
            unresolvedConstraints: [],
            selectedArtifactReferences: generation.lastCheckpointArtifactID.map { [$0] } ?? [],
            lastValidatedAggregateStateJSON: nil,
            ownerAndBindingContextJSON: contextData,
            scopeContextJSON: scopeData,
            compactedConversationStateJSON: nil
        )
    }

    // MARK: - Private Helpers

    private static func buildOwnerContext(ownerKey: String?, scope: SessionReuseScope?, familyID: String?) -> [String: String] {
        [
            "ownerKey": ownerKey ?? "unknown",
            "scope": scope?.rawValue ?? "unknown",
            "familyID": familyID ?? "nil"
        ]
    }

    private static func buildScopeContextData(scope: SessionReuseScope?, familyID: String?) -> Data? {
        let scopeContext: [String: String] = [
            "scope": scope?.rawValue ?? "unknown",
            "familyID": familyID ?? "nil"
        ]
        return try? JSONEncoder().encode(scopeContext)
    }
}
