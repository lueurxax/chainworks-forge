import Foundation

nonisolated enum AgentCanonicalOutcome: String, Codable, Sendable, Equatable, Hashable {
    case completed = "completed"
    case completedWithTransportError = "completed_with_transport_error"
    case failedBeforeOutput = "failed_before_output"
    case failedAfterOutputValidation = "failed_after_output_validation"
    case timedOutBeforeOutput = "timed_out_before_output"
    case timedOutAfterOutput = "timed_out_after_output"
    case cancelledBeforeOutput = "cancelled_before_output"
    case cancelledAfterOutput = "cancelled_after_output"
    case limitExhaustedBeforeOutput = "limit_exhausted_before_output"
    case limitExhaustedAfterOutput = "limit_exhausted_after_output"
}

extension AgentCanonicalOutcome {
    nonisolated var coarseStatus: AgentStatus {
        switch self {
        case .completed, .completedWithTransportError:
            return .completed
        case .failedBeforeOutput,
             .failedAfterOutputValidation,
             .timedOutBeforeOutput,
             .timedOutAfterOutput,
             .limitExhaustedBeforeOutput,
             .limitExhaustedAfterOutput:
            return .failed
        case .cancelledBeforeOutput, .cancelledAfterOutput:
            return .cancelled
        }
    }

    nonisolated var blocksForwardProgress: Bool {
        switch self {
        case .completed:
            return false
        case .completedWithTransportError,
             .failedBeforeOutput,
             .failedAfterOutputValidation,
             .timedOutBeforeOutput,
             .timedOutAfterOutput,
             .limitExhaustedBeforeOutput,
             .limitExhaustedAfterOutput:
            return true
        case .cancelledBeforeOutput, .cancelledAfterOutput:
            return false
        }
    }
}

nonisolated enum TransportErrorKind: String, Codable, Sendable, Equatable {
    case timeout = "timeout"
    case stream = "stream"
    case provider = "provider"
    case unknown = "unknown"
}

nonisolated enum OutputPresence: String, Codable, Sendable, Equatable {
    case none = "none"
    case durableOutput = "durable_output"
}

nonisolated enum StageSettlementKind: String, Codable, Sendable, Equatable {
    case completed = "completed"
    case blocked = "blocked"
    case failed = "failed"
    case repaired = "repaired"
    case superseded = "superseded"
}

nonisolated struct OutcomeEnvelope: Codable, Sendable, Equatable {
    let canonicalOutcome: AgentCanonicalOutcome?
    let transportErrorKind: TransportErrorKind?
    let providerStopReason: String?
    let outputPresence: OutputPresence
    let rawErrorMessage: String?
    let rawFinishEvent: String?
}

nonisolated enum RuntimeBindingTrustLevel: String, Codable, Sendable, Equatable {
    case fixtureVerified = "fixture_verified"
    case serverVerified = "server_verified"
    case serverUnverified = "server_unverified"
    case unverifiable = "unverifiable"
    case unknown = "unknown"
}

nonisolated struct RuntimeBindingResolution: Sendable, Equatable {
    let provider: String
    let model: String
    let trustLevel: RuntimeBindingTrustLevel
}

nonisolated struct RuntimeBindingTruthSummaryRow: Sendable, Equatable {
    let agentID: String
    let frozenProvider: String
    let frozenModel: String
    let runtimeProvider: String
    let runtimeModel: String
    let trustLevel: RuntimeBindingTrustLevel

    var hasMeaningfulDelta: Bool {
        trustLevel != .serverVerified
            || frozenProvider != runtimeProvider
            || frozenModel != runtimeModel
    }

    var summaryLine: String {
        "\(agentID): frozen=\(frozenProvider)/\(frozenModel) runtime=\(runtimeProvider)/\(runtimeModel) [\(trustLevel.rawValue)]"
    }
}

nonisolated enum RuntimeBindingTruthResolver {
    static func resolve(
        agent: AgentExecution,
        frozenBinding: ResolvedProviderBinding?,
        frozenProvenance: FrozenBindingProvenance?
    ) -> RuntimeBindingResolution {
        let receipt = decodeReceipt(from: agent)
        let receiptProvider = nonEmpty(receipt?.providerFamily)
        let receiptModel = nonEmpty(receipt?.model)

        let runtimeProvider = nonEmpty(agent.runtimeProvider)
        let runtimeModel = nonEmpty(agent.runtimeModel)

        let frozenProvider = nonEmpty(frozenBinding?.providerFamily)
            ?? nonEmpty(frozenProvenance?.resolvedProviderFamily)
            ?? nonEmpty(agent.provider)
        let frozenModel = nonEmpty(frozenBinding?.model)
            ?? nonEmpty(frozenProvenance?.resolvedModel)
            ?? nonEmpty(agent.resolvedModel)
            ?? nonEmpty(agent.resolvedBackendProfileID)

        let provider = runtimeProvider ?? receiptProvider ?? frozenProvider ?? "unknown"
        let model = runtimeModel ?? receiptModel ?? frozenModel ?? "unknown"

        let trustLevel: RuntimeBindingTrustLevel
        if runtimeProvider != nil || runtimeModel != nil {
            if let receiptProvider, let receiptModel {
                let providerMatches = (runtimeProvider ?? receiptProvider) == receiptProvider
                let modelMatches = (runtimeModel ?? receiptModel) == receiptModel
                trustLevel = (providerMatches && modelMatches) ? .serverVerified : .unverifiable
            } else {
                trustLevel = .unverifiable
            }
        } else if receiptProvider != nil, receiptModel != nil {
            trustLevel = .unverifiable
        } else if agent.providerReceiptJSON != nil
            || runtimeProvider != nil
            || runtimeModel != nil
            || frozenBinding != nil
            || frozenProvenance != nil {
            trustLevel = .unverifiable
        } else {
            trustLevel = .unknown
        }

        return RuntimeBindingResolution(
            provider: provider,
            model: model,
            trustLevel: trustLevel
        )
    }

    static func deriveRunTrustLevel(
        agents: [AgentExecution],
        frozenBindings: [String: ResolvedProviderBinding] = [:],
        frozenProvenances: [String: FrozenBindingProvenance] = [:],
        persisted: String? = nil
    ) -> String {
        if persisted == RuntimeBindingTrustLevel.fixtureVerified.rawValue {
            return RuntimeBindingTrustLevel.fixtureVerified.rawValue
        }

        let resolutions = agents.map {
            resolve(
                agent: $0,
                frozenBinding: frozenBindings[$0.agentID],
                frozenProvenance: frozenProvenances[$0.agentID]
            )
        }

        guard !resolutions.isEmpty else { return persisted ?? RuntimeBindingTrustLevel.unknown.rawValue }

        if resolutions.allSatisfy({ $0.trustLevel == .serverVerified }) {
            return RuntimeBindingTrustLevel.serverVerified.rawValue
        }

        if resolutions.contains(where: { $0.trustLevel == .unverifiable || $0.trustLevel == .serverVerified }) {
            return RuntimeBindingTrustLevel.unverifiable.rawValue
        }

        return persisted ?? RuntimeBindingTrustLevel.unknown.rawValue
    }

    private static func decodeReceipt(from agent: AgentExecution) -> ProviderExecutionReceipt? {
        guard let data = agent.providerReceiptJSON else { return nil }
        return try? JSONDecoder().decode(ProviderExecutionReceipt.self, from: data)
    }

    private static func nonEmpty(_ value: String?) -> String? {
        guard let value else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

enum RuntimeBindingTruthSummaryBuilder {
    static func rows(for run: Run) -> [RuntimeBindingTruthSummaryRow] {
        let frozenBindings = decodeFrozenBindings(from: run)
        let frozenProvenances = decodeFrozenProvenances(from: run)

        return canonicalAgents(in: run).map { agent in
            let frozenBinding = frozenBindings[agent.agentID]
            let frozenProvenance = frozenProvenances[agent.agentID]
            let resolution = RuntimeBindingTruthResolver.resolve(
                agent: agent,
                frozenBinding: frozenBinding,
                frozenProvenance: frozenProvenance
            )

            let frozenProvider = nonEmpty(frozenBinding?.providerFamily)
                ?? nonEmpty(frozenProvenance?.resolvedProviderFamily)
                ?? nonEmpty(agent.provider)
                ?? "unknown"
            let frozenModel = nonEmpty(frozenBinding?.model)
                ?? nonEmpty(frozenProvenance?.resolvedModel)
                ?? nonEmpty(agent.resolvedModel)
                ?? nonEmpty(agent.resolvedBackendProfileID)
                ?? "unknown"

            return RuntimeBindingTruthSummaryRow(
                agentID: agent.agentID,
                frozenProvider: frozenProvider,
                frozenModel: frozenModel,
                runtimeProvider: resolution.provider,
                runtimeModel: resolution.model,
                trustLevel: resolution.trustLevel
            )
        }
    }

    static func summaryText(for run: Run, limit: Int = 3) -> String? {
        let interestingRows = rows(for: run).filter(\.hasMeaningfulDelta)
        guard !interestingRows.isEmpty else { return nil }

        let rendered = interestingRows.prefix(limit).map(\.summaryLine).joined(separator: " | ")
        if interestingRows.count > limit {
            return rendered + " | +\(interestingRows.count - limit) more"
        }
        return rendered
    }

    private static func canonicalAgents(in run: Run) -> [AgentExecution] {
        let grouped = Dictionary(grouping: run.stageExecutions.flatMap(\.agentExecutions), by: \.agentID)
        return grouped.values.compactMap { executions in
            executions.max { lhs, rhs in
                let lhsAttempt = lhs.agentAttemptNumber ?? 1
                let rhsAttempt = rhs.agentAttemptNumber ?? 1
                if lhsAttempt != rhsAttempt {
                    return lhsAttempt < rhsAttempt
                }
                if lhs.startedAt != rhs.startedAt {
                    return lhs.startedAt < rhs.startedAt
                }
                return lhs.id.uuidString < rhs.id.uuidString
            }
        }
        .sorted { $0.agentID < $1.agentID }
    }

    private static func decodeFrozenBindings(from run: Run) -> [String: ResolvedProviderBinding] {
        guard let data = run.providerBindingSnapshotJSON else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data)) ?? [:]
    }

    private static func decodeFrozenProvenances(from run: Run) -> [String: FrozenBindingProvenance] {
        guard let data = run.bindingProvenanceJSON else { return [:] }
        return (try? JSONDecoder().decode([String: FrozenBindingProvenance].self, from: data)) ?? [:]
    }

    private static func nonEmpty(_ value: String?) -> String? {
        guard let value else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

extension AgentExecution {
    var blocksForwardProgress: Bool {
        if let canonicalOutcome {
            return canonicalOutcome.blocksForwardProgress
        }
        return status == .failed
    }

    var isAggregateProposalReviewStep: Bool {
        taskName == "aggregate_proposal_reviews"
    }
}

enum ExecutionTruthSupport {
    static func persistTerminalTruth(
        for agentExec: AgentExecution,
        canonicalOutcome: AgentCanonicalOutcome,
        transportErrorKind: TransportErrorKind?,
        providerStopReason: String?,
        outputPresence: OutputPresence,
        runtimeProvider: String?,
        runtimeModel: String?,
        rawErrorMessage: String? = nil,
        rawFinishEvent: String? = nil,
        envelope: OutcomeEnvelope? = nil,
        frozenBindings: [String: ResolvedProviderBinding] = [:],
        frozenProvenances: [String: FrozenBindingProvenance] = [:]
    ) {
        agentExec.status = canonicalOutcome.coarseStatus
        agentExec.canonicalOutcome = canonicalOutcome
        agentExec.transportErrorKind = transportErrorKind
        agentExec.providerStopReason = providerStopReason
        agentExec.outputPresence = outputPresence
        agentExec.runtimeProvider = runtimeProvider
        agentExec.runtimeModel = runtimeModel
        agentExec.settledAt = agentExec.completedAt ?? Date()

        let resolvedEnvelope = envelope ?? OutcomeEnvelope(
            canonicalOutcome: canonicalOutcome,
            transportErrorKind: transportErrorKind,
            providerStopReason: providerStopReason,
            outputPresence: outputPresence,
            rawErrorMessage: rawErrorMessage,
            rawFinishEvent: rawFinishEvent
        )
        agentExec.outcomeEnvelopeJSON = encodeOutcomeEnvelope(resolvedEnvelope)

        if let run = agentExec.stageExecution?.run {
            run.runtimeTrustLevel = RuntimeBindingTruthResolver.deriveRunTrustLevel(
                agents: run.stageExecutions.flatMap(\.agentExecutions),
                frozenBindings: frozenBindings,
                frozenProvenances: frozenProvenances,
                persisted: run.runtimeTrustLevel
            )
        }
    }

    static func encodedOutcomeEnvelope(
        canonicalOutcome: AgentCanonicalOutcome,
        transportErrorKind: TransportErrorKind?,
        providerStopReason: String?,
        outputPresence: OutputPresence,
        rawErrorMessage: String? = nil,
        rawFinishEvent: String? = nil
    ) -> Data? {
        encodeOutcomeEnvelope(
            OutcomeEnvelope(
                canonicalOutcome: canonicalOutcome,
                transportErrorKind: transportErrorKind,
                providerStopReason: providerStopReason,
                outputPresence: outputPresence,
                rawErrorMessage: rawErrorMessage,
                rawFinishEvent: rawFinishEvent
            )
        )
    }

    nonisolated static func derivedOutputPresence(for agentExec: AgentExecution) -> OutputPresence {
        if let outputPresence = agentExec.outputPresence {
            return outputPresence
        }
        let envelopes = decodeOutputEnvelopes(from: agentExec)
        if envelopes.contains(where: { $0.rawPayloadPersisted || $0.normalizedArtifactProduced }) {
            return .durableOutput
        }
        return .none
    }

    nonisolated static func decodedReceipt(from agentExec: AgentExecution) -> ProviderExecutionReceipt? {
        guard let data = agentExec.providerReceiptJSON else { return nil }
        return try? JSONDecoder().decode(ProviderExecutionReceipt.self, from: data)
    }

    nonisolated static func decodeOutputEnvelopes(from agentExec: AgentExecution) -> [StructuredOutputEnvelope] {
        guard let data = agentExec.outputEnvelopesJSON else { return [] }
        return (try? JSONDecoder().decode([StructuredOutputEnvelope].self, from: data)) ?? []
    }

    nonisolated static func hasValidationFailure(_ agentExec: AgentExecution) -> Bool {
        guard let data = agentExec.validationFailureJSON else { return false }
        return (try? JSONDecoder().decode(ValidationFailureRecord.self, from: data)) != nil
    }

    nonisolated static func isLimitExhaustionReason(_ reason: String?) -> Bool {
        guard let reason else { return false }
        let normalized = reason.lowercased()
        return normalized.contains("max_tokens")
            || normalized.contains("max token")
            || normalized.contains("rate_limit")
            || normalized.contains("rate limit")
            || normalized.contains("quota")
            || normalized.contains("budget")
            || normalized.contains("limit")
    }

    nonisolated static func isPolicyBoundStopReason(_ reason: String?) -> Bool {
        guard let reason else { return false }
        let normalized = reason.lowercased()
        return normalized.contains("policy")
            || normalized.contains("safety")
            || normalized.contains("blocked")
            || normalized.contains("blocklist")
            || normalized.contains("prohibited")
            || normalized.contains("content_filter")
            || normalized.contains("content filter")
    }

    nonisolated static func deterministicLegacyOutcome(for agentExec: AgentExecution) -> AgentCanonicalOutcome? {
        if let canonicalOutcome = agentExec.canonicalOutcome {
            return canonicalOutcome
        }

        let outputPresence = derivedOutputPresence(for: agentExec)
        var candidates: [AgentCanonicalOutcome] = []
        let limitExhaustion = isLimitExhaustionReason(agentExec.providerStopReason)
        let policyBound = isPolicyBoundStopReason(agentExec.providerStopReason)

        if agentExec.status == .cancelled {
            candidates.append(outputPresence == .durableOutput ? .cancelledAfterOutput : .cancelledBeforeOutput)
        }

        if hasValidationFailure(agentExec), outputPresence == .durableOutput {
            candidates.append(.failedAfterOutputValidation)
        }

        if limitExhaustion {
            candidates.append(outputPresence == .durableOutput ? .limitExhaustedAfterOutput : .limitExhaustedBeforeOutput)
        }

        if let transportErrorKind = agentExec.transportErrorKind, !limitExhaustion, !policyBound {
            switch (transportErrorKind, outputPresence) {
            case (.timeout, .durableOutput):
                candidates.append(.timedOutAfterOutput)
            case (.timeout, .none):
                candidates.append(.timedOutBeforeOutput)
            case (_, .durableOutput):
                candidates.append(.completedWithTransportError)
            case (_, .none):
                candidates.append(.failedBeforeOutput)
            }
        }

        if agentExec.status == .completed && outputPresence == .durableOutput {
            candidates.append(.completed)
        }

        let unique = Array(Set(candidates))
        guard unique.count == 1 else { return nil }
        return unique[0]
    }

    private static func encodeOutcomeEnvelope(_ envelope: OutcomeEnvelope?) -> Data? {
        guard let envelope else { return nil }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return try? encoder.encode(envelope)
    }
}
