import Foundation

// MARK: - GooseAgentExecutor (concrete AgentExecutor via Goose — Section 8.1)

/// Concrete implementation of `AgentExecutor` using a Goose backend.
/// Each execution creates an isolated session via `GooseSessionBridge` (ARCH-027).
///
/// Responsibilities:
/// 1. Build the execution packet.
/// 2. Create an isolated Goose session.
/// 3. Bind prompt + task + input artifact references.
/// 4. Stream execution events.
/// 5. Persist raw transcript/receipt artifacts.
/// 6. Extract declared output artifacts.
/// 7. Return a structured `AgentResult`.
final class GooseAgentExecutor: AgentExecutor, @unchecked Sendable {

    // MARK: - Dependencies

    let sessionBridge: GooseSessionBridge
    let override: LiveExecutionOverride?

    /// Callback for live execution events (for UI streaming).
    /// Called on arbitrary threads — the UI layer must dispatch to MainActor.
    var onExecutionEvent: (@Sendable (String, ExecutionEvent) -> Void)?

    // MARK: - Init

    /// Proposal 005: accepts `any GooseTransportProtocol` instead of concrete `GooseTransport`.
    init(
        transport: any GooseTransportProtocol,
        override: LiveExecutionOverride? = nil
    ) {
        self.sessionBridge = GooseSessionBridge(transport: transport)
        self.override = override
    }

    // MARK: - AgentExecutor Protocol

    /// Proposal 013: Maximum transport-level retry attempts for timeout errors.
    private static let maxTransportRetries = 1

    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult {
        // Proposal 013: Retry on transport timeout to handle sequential Goose processing
        var lastResult: AgentResult?
        for attempt in 0...Self.maxTransportRetries {
            let result = try await executeAttempt(
                task: task,
                agent: agent,
                context: context,
                attemptIndex: attempt
            )

            // If succeeded or not a timeout error, return immediately
            if result.succeeded || !isTimeoutError(result) {
                return result
            }

            lastResult = result

            // Only retry on timeout, and only once
            if attempt < Self.maxTransportRetries {
                // Brief pause before retry to let server recover
                try? await Task.sleep(for: .seconds(2))
            }
        }
        return lastResult!
    }

    /// Check if the result indicates a transport timeout.
    private func isTimeoutError(_ result: AgentResult) -> Bool {
        guard let error = result.errorMessage else { return false }
        return error.contains("timed out") || error.contains("-1001") || error.contains("timeout")
    }

    private func executeAttempt(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext,
        attemptIndex: Int
    ) async throws -> AgentResult {
        let startedAt = Date()
        // Proposal 013: V2 resolver — catalog-driven contract resolution
        let expectedOutputs = OutputContractResolverV2.expectedOutputs(for: task, agent: agent)

        // Step 1: Validate workspace (ARCH-025)
        try GooseSessionBridge.validateWorkspace(context.workspace)

        // Step 2: Create isolated session and start execution
        let sessionExecution: GooseSessionExecution
        do {
            sessionExecution = try await sessionBridge.executeInIsolatedSession(
                agent: agent,
                task: task,
                context: context,
                override: override
            )
        } catch {
            return AgentResult(
                outputs: [:],
                logSnippet: "Session creation failed (attempt \(attemptIndex)): \(error.localizedDescription)",
                costCents: nil,
                succeeded: false,
                errorMessage: "Session creation failed: \(error.localizedDescription)",
                sessionID: nil,
                durationSeconds: Date().timeIntervalSince(startedAt),
                providerReceipt: nil,
                resolvedModel: context.providerBinding?.model ?? override?.model ?? agent.model,
                configuredProviderID: context.providerBinding?.configuredProviderID,
                adapterVersion: context.providerBinding?.adapterVersion
            )
        }

        // Step 3: Process the event stream
        let eventBridge = ExecutionEventBridge()
        let agentID = agent.id
        let onEvent = onExecutionEvent

        let streamResult: ExecutionStreamResult
        do {
            streamResult = try await eventBridge.processStream(
                sessionExecution.eventStream,
                onEvent: { event in
                    onEvent?(agentID, event)
                }
            )
        } catch {
            // Stream failed — still close the session and check for files on disk.
            // Proposal 005 fix: goosed agents write files via developer tools BEFORE
            // the SSE stream completes. If the stream errors (timeout, disconnect),
            // the agent's output may already be on disk. We must still collect it
            // and generate a receipt for audit evidence.
            await sessionExecution.closeSession()

            let completedAt = Date()
            let resolvedProvider = override?.provider ?? agent.provider
            let resolvedModel = override?.model ?? agent.model
            let resolvedEffort = override?.effort ?? agent.effort

            // Check for files the agent already wrote to disk
            var salvaged: [String: Data] = [:]
            let outputDir = context.workspace.artifactRoot
                .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
                .appendingPathComponent(agent.id, isDirectory: true)
                .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)

            for outputName in expectedOutputs {
                let outputPath = outputDir.appendingPathComponent(outputName)
                if FileManager.default.fileExists(atPath: outputPath.path),
                   let data = try? Data(contentsOf: outputPath) {
                    salvaged[outputName] = data
                }
            }

            // Generate receipt even on stream failure
            let outputPresence: OutputPresence = salvaged.contains { key, _ in
                !key.hasSuffix("_receipt.json") && !key.hasSuffix("_transcript.md")
            } ? .durableOutput : .none
            let transportKind = classifyTransportErrorKind(error.localizedDescription)
            let canonicalOutcome = classifyStreamFailureOutcome(
                errorMessage: error.localizedDescription,
                outputPresence: outputPresence
            )
            let failureMessage = failureMessage(
                for: canonicalOutcome,
                fallback: "Stream processing failed: \(error.localizedDescription)"
            )
            let receiptArtifacts = ExecutionReceiptBuilder.buildReceipt(
                agentID: agent.id,
                sessionID: sessionExecution.sessionID,
                stageID: context.stageID,
                iteration: context.iteration,
                attemptNumber: context.attemptNumber,
                startedAt: startedAt,
                completedAt: completedAt,
                events: eventBridge.eventLog,
                toolCalls: eventBridge.toolCalls,
                finalContent: nil,
                succeeded: canonicalOutcome == .completed,
                errorMessage: failureMessage,
                provider: resolvedProvider,
                model: resolvedModel,
                effort: resolvedEffort
            )
            for (name, data) in receiptArtifacts {
                salvaged[name] = data
            }

            return AgentResult(
                outputs: salvaged,
                logSnippet: "Stream failed but salvaged \(salvaged.count) artifacts from disk. Error: \(error.localizedDescription)",
                costCents: nil,
                succeeded: canonicalOutcome == .completed,
                errorMessage: failureMessage,
                sessionID: sessionExecution.sessionID,
                durationSeconds: completedAt.timeIntervalSince(startedAt),
                providerReceipt: UsageReceiptNormalizer.makeReceipt(
                    providerFamily: context.providerBinding?.providerFamily ?? resolvedProvider,
                    configuredProviderID: context.providerBinding?.configuredProviderID,
                    model: context.providerBinding?.model ?? resolvedModel,
                    effort: context.providerBinding?.effort ?? resolvedEffort,
                    transport: context.providerBinding?.transport ?? "goose",
                    costCents: nil,
                    durationSeconds: completedAt.timeIntervalSince(startedAt),
                    rawReceiptJSON: receiptArtifacts["\(agent.id)_receipt.json"]
                ),
                resolvedModel: context.providerBinding?.model ?? resolvedModel,
                configuredProviderID: context.providerBinding?.configuredProviderID,
                adapterVersion: context.providerBinding?.adapterVersion,
                canonicalOutcome: canonicalOutcome,
                transportErrorKind: transportKind,
                providerStopReason: nil,
                outputPresence: outputPresence,
                runtimeProvider: context.providerBinding?.providerFamily ?? resolvedProvider,
                runtimeModel: context.providerBinding?.model ?? resolvedModel,
                outcomeEnvelope: OutcomeEnvelope(
                    canonicalOutcome: canonicalOutcome,
                    transportErrorKind: transportKind,
                    providerStopReason: nil,
                    outputPresence: outputPresence,
                    rawErrorMessage: error.localizedDescription,
                    rawFinishEvent: nil
                )
            )
        }

        let completedAt = Date()

        // Step 4: Close the session
        await sessionExecution.closeSession()

        // Step 5: Build receipt/transcript artifacts (ARCH-032)
        let resolvedProvider = override?.provider ?? agent.provider
        let resolvedModel = override?.model ?? agent.model
        let resolvedEffort = override?.effort ?? agent.effort

        // Step 6: Extract declared output artifacts from workspace
        var outputs: [String: Data] = [:]

        // Try to read expected output files from the artifact directory
        let outputDir = context.workspace.artifactRoot
            .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
            .appendingPathComponent(agent.id, isDirectory: true)
            .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)

        for outputName in expectedOutputs {
            let outputPath = outputDir.appendingPathComponent(outputName)
            if FileManager.default.fileExists(atPath: outputPath.path) {
                do {
                    let data = try Data(contentsOf: outputPath)
                    outputs[outputName] = data
                } catch {
                    // Log but continue — will be caught by validation below
                    print("Warning: Could not read output '\(outputName)': \(error.localizedDescription)")
                }
            }
        }

        // If the final output contains content but no files were written,
        // try to use the final content as the primary output
        if outputs.isEmpty, let content = streamResult.finalContent, !content.isEmpty {
            if let primaryOutput = expectedOutputs.first {
                outputs[primaryOutput] = content.data(using: .utf8) ?? Data()
            }
        }

        let outputPresence: OutputPresence = outputs.isEmpty ? .none : .durableOutput
        let canonicalOutcome = classifyCompletedStreamOutcome(
            outputPresence: outputPresence,
            finishReason: streamResult.finishReason,
            hadExplicitFinalOutput: streamResult.finalContent != nil
        )
        let failureMessage = canonicalOutcome == .completed ? nil : failureMessage(
            for: canonicalOutcome,
            fallback: "Execution did not produce final output"
        )

        // Step 7: Validate required outputs (Section 8.3)
        let missingOutputs = expectedOutputs.filter { outputs[$0] == nil }
        let finalCanonicalOutcome: AgentCanonicalOutcome
        let finalErrorMessage: String?

        if !missingOutputs.isEmpty {
            finalCanonicalOutcome = canonicalOutcome == .completed ? .failedBeforeOutput : canonicalOutcome
            finalErrorMessage = "Required outputs missing: \(missingOutputs.joined(separator: ", "))"
        } else {
            finalCanonicalOutcome = canonicalOutcome
            finalErrorMessage = failureMessage
        }

        let receiptArtifacts = ExecutionReceiptBuilder.buildReceipt(
            agentID: agent.id,
            sessionID: sessionExecution.sessionID,
            stageID: context.stageID,
            iteration: context.iteration,
            attemptNumber: context.attemptNumber,
            startedAt: startedAt,
            completedAt: completedAt,
            events: eventBridge.eventLog,
            toolCalls: eventBridge.toolCalls,
            finalContent: streamResult.finalContent,
            succeeded: finalCanonicalOutcome == .completed,
            errorMessage: finalErrorMessage,
            provider: resolvedProvider,
            model: resolvedModel,
            effort: resolvedEffort
        )

        for (name, data) in receiptArtifacts {
            outputs[name] = data
        }

        if !missingOutputs.isEmpty {
            // Stage should fail loudly — silent success is worse than a visible crash
            let missingList = missingOutputs.joined(separator: ", ")
            return AgentResult(
                outputs: outputs, // Still include receipts/transcripts for debugging
                logSnippet: "Missing required outputs: \(missingList). Session: \(sessionExecution.sessionID)",
                costCents: estimateCost(streamResult: streamResult),
                succeeded: false,
                errorMessage: "Required outputs missing: \(missingList)",
                sessionID: sessionExecution.sessionID,
                durationSeconds: completedAt.timeIntervalSince(startedAt),
                providerReceipt: UsageReceiptNormalizer.makeReceipt(
                    providerFamily: context.providerBinding?.providerFamily ?? resolvedProvider,
                    configuredProviderID: context.providerBinding?.configuredProviderID,
                    model: context.providerBinding?.model ?? resolvedModel,
                    effort: context.providerBinding?.effort ?? resolvedEffort,
                    transport: context.providerBinding?.transport ?? "goose",
                    costCents: estimateCost(streamResult: streamResult),
                    durationSeconds: completedAt.timeIntervalSince(startedAt),
                    rawReceiptJSON: receiptArtifacts["\(agent.id)_receipt.json"]
                ),
                resolvedModel: context.providerBinding?.model ?? resolvedModel,
                configuredProviderID: context.providerBinding?.configuredProviderID,
                adapterVersion: context.providerBinding?.adapterVersion,
                canonicalOutcome: finalCanonicalOutcome,
                transportErrorKind: nil,
                providerStopReason: streamResult.finishReason,
                outputPresence: outputPresence,
                runtimeProvider: context.providerBinding?.providerFamily ?? resolvedProvider,
                runtimeModel: context.providerBinding?.model ?? resolvedModel,
                outcomeEnvelope: OutcomeEnvelope(
                    canonicalOutcome: finalCanonicalOutcome,
                    transportErrorKind: nil,
                    providerStopReason: streamResult.finishReason,
                    outputPresence: outputPresence,
                    rawErrorMessage: "Required outputs missing: \(missingList)",
                    rawFinishEvent: streamResult.finishRaw
                )
            )
        }

        // Step 8: Return successful result
        let logSnippet = buildLogSnippet(
            agent: agent,
            sessionID: sessionExecution.sessionID,
            streamResult: streamResult,
            startedAt: startedAt,
            completedAt: completedAt
        )

        return AgentResult(
            outputs: outputs,
            logSnippet: logSnippet,
            costCents: estimateCost(streamResult: streamResult),
            succeeded: canonicalOutcome == .completed,
            errorMessage: failureMessage,
            sessionID: sessionExecution.sessionID,
            durationSeconds: completedAt.timeIntervalSince(startedAt),
            providerReceipt: UsageReceiptNormalizer.makeReceipt(
                providerFamily: context.providerBinding?.providerFamily ?? resolvedProvider,
                configuredProviderID: context.providerBinding?.configuredProviderID,
                model: context.providerBinding?.model ?? resolvedModel,
                effort: context.providerBinding?.effort ?? resolvedEffort,
                transport: context.providerBinding?.transport ?? "goose",
                costCents: estimateCost(streamResult: streamResult),
                durationSeconds: completedAt.timeIntervalSince(startedAt),
                rawReceiptJSON: receiptArtifacts["\(agent.id)_receipt.json"]
            ),
            resolvedModel: context.providerBinding?.model ?? resolvedModel,
            configuredProviderID: context.providerBinding?.configuredProviderID,
            adapterVersion: context.providerBinding?.adapterVersion,
            canonicalOutcome: finalCanonicalOutcome,
            transportErrorKind: nil,
            providerStopReason: streamResult.finishReason,
            outputPresence: outputPresence,
            runtimeProvider: context.providerBinding?.providerFamily ?? resolvedProvider,
            runtimeModel: context.providerBinding?.model ?? resolvedModel,
            outcomeEnvelope: OutcomeEnvelope(
                canonicalOutcome: finalCanonicalOutcome,
                transportErrorKind: nil,
                providerStopReason: streamResult.finishReason,
                outputPresence: outputPresence,
                rawErrorMessage: finalErrorMessage,
                rawFinishEvent: streamResult.finishRaw
            )
        )
    }

    // MARK: - Private Helpers

    private func buildLogSnippet(
        agent: ResolvedAgent,
        sessionID: String,
        streamResult: ExecutionStreamResult,
        startedAt: Date,
        completedAt: Date
    ) -> String {
        let duration = String(format: "%.1f", completedAt.timeIntervalSince(startedAt))
        let toolCount = streamResult.toolCalls.count
        return "Live execution of '\(agent.id)' completed in \(duration)s. " +
               "Session: \(sessionID). Tool calls: \(toolCount)."
    }

    private func estimateCost(streamResult: ExecutionStreamResult) -> Int64? {
        // Rough cost estimation based on text length
        // In production, this would come from the provider's usage API
        let inputChars = streamResult.accumulatedText.count
        let estimatedTokens = inputChars / 4
        // Rough: $0.01 per 1000 tokens = 1 cent per 1000 tokens
        return max(1, Int64(estimatedTokens / 1000))
    }

    private func classifyTransportErrorKind(_ errorMessage: String) -> TransportErrorKind {
        let lowercased = errorMessage.lowercased()
        if lowercased.contains("timed out") || lowercased.contains("timeout") || lowercased.contains("-1001") {
            return .timeout
        }
        if lowercased.contains("provider") {
            return .provider
        }
        if lowercased.contains("stream") {
            return .stream
        }
        return .unknown
    }

    private func classifyStreamFailureOutcome(
        errorMessage: String,
        outputPresence: OutputPresence
    ) -> AgentCanonicalOutcome {
        switch (classifyTransportErrorKind(errorMessage), outputPresence) {
        case (.timeout, .durableOutput):
            return .timedOutAfterOutput
        case (.timeout, .none):
            return .timedOutBeforeOutput
        case (_, .durableOutput):
            return .completedWithTransportError
        case (_, .none):
            return .failedBeforeOutput
        }
    }

    private func classifyCompletedStreamOutcome(
        outputPresence: OutputPresence,
        finishReason: String?,
        hadExplicitFinalOutput: Bool
    ) -> AgentCanonicalOutcome {
        guard let finishReason else {
            return outputPresence == .durableOutput ? .completed : .failedBeforeOutput
        }

        if isLimitExhaustionReason(finishReason) {
            return outputPresence == .durableOutput ? .limitExhaustedAfterOutput : .limitExhaustedBeforeOutput
        }

        if hadExplicitFinalOutput {
            return .completed
        }

        if outputPresence == .durableOutput {
            return .completed
        }

        if isNeutralFinishReason(finishReason) {
            return .failedBeforeOutput
        }

        return .failedBeforeOutput
    }

    private func isLimitExhaustionReason(_ reason: String) -> Bool {
        let normalized = reason.lowercased()
        return normalized.contains("max_tokens")
            || normalized.contains("max token")
            || normalized.contains("rate_limit")
            || normalized.contains("rate limit")
            || normalized.contains("quota")
            || normalized.contains("budget")
            || normalized.contains("limit")
    }

    private func isNeutralFinishReason(_ reason: String) -> Bool {
        let normalized = reason.lowercased()
        return normalized == "stop" || normalized == "session_closed" || normalized == "session closed"
    }

    private func failureMessage(
        for outcome: AgentCanonicalOutcome,
        fallback: String
    ) -> String {
        switch outcome {
        case .limitExhaustedBeforeOutput:
            return "Provider or app limit exhausted before output was produced"
        case .limitExhaustedAfterOutput:
            return "Provider or app limit exhausted after output was produced"
        case .timedOutBeforeOutput:
            return "Execution timed out before output was produced"
        case .timedOutAfterOutput:
            return "Execution timed out after output was produced"
        case .completedWithTransportError:
            return "Execution produced durable output but transport errored afterward"
        case .failedBeforeOutput:
            return fallback
        case .failedAfterOutputValidation:
            return "Output validation failed after output was produced"
        case .cancelledBeforeOutput:
            return "Execution was cancelled before output was produced"
        case .cancelledAfterOutput:
            return "Execution was cancelled after output was produced"
        case .completed:
            return fallback
        }
    }
}
