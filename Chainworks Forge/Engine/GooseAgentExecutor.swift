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

    init(
        transport: GooseTransport,
        override: LiveExecutionOverride? = nil
    ) {
        self.sessionBridge = GooseSessionBridge(transport: transport)
        self.override = override
    }

    // MARK: - AgentExecutor Protocol

    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult {
        let startedAt = Date()

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
                logSnippet: "Session creation failed: \(error.localizedDescription)",
                costCents: nil,
                succeeded: false,
                errorMessage: "Session creation failed: \(error.localizedDescription)"
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
            // Stream failed — still close the session
            await sessionExecution.closeSession()

            return AgentResult(
                outputs: [:],
                logSnippet: "Stream processing failed: \(error.localizedDescription)",
                costCents: nil,
                succeeded: false,
                errorMessage: "Stream processing failed: \(error.localizedDescription)"
            )
        }

        let completedAt = Date()

        // Step 4: Close the session
        await sessionExecution.closeSession()

        // Step 5: Build receipt/transcript artifacts (ARCH-032)
        let resolvedProvider = override?.provider ?? agent.provider
        let resolvedModel = override?.model ?? agent.model
        let resolvedEffort = override?.effort ?? agent.effort

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
            succeeded: streamResult.succeeded,
            errorMessage: streamResult.succeeded ? nil : "Execution did not produce final output",
            provider: resolvedProvider,
            model: resolvedModel,
            effort: resolvedEffort
        )

        // Step 6: Extract declared output artifacts from workspace
        var outputs: [String: Data] = [:]

        // Try to read expected output files from the artifact directory
        let outputDir = context.workspace.artifactRoot
            .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
            .appendingPathComponent(agent.id, isDirectory: true)
            .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)

        for outputName in agent.outputs {
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
            if let primaryOutput = agent.outputs.first {
                outputs[primaryOutput] = content.data(using: .utf8) ?? Data()
            }
        }

        // Add receipt artifacts to outputs
        for (name, data) in receiptArtifacts {
            outputs[name] = data
        }

        // Step 7: Validate required outputs (Section 8.3)
        let missingOutputs = agent.outputs.filter { outputs[$0] == nil }

        if !missingOutputs.isEmpty {
            // Stage should fail loudly — silent success is worse than a visible crash
            let missingList = missingOutputs.joined(separator: ", ")
            return AgentResult(
                outputs: outputs, // Still include receipts/transcripts for debugging
                logSnippet: "Missing required outputs: \(missingList). Session: \(sessionExecution.sessionID)",
                costCents: estimateCost(streamResult: streamResult),
                succeeded: false,
                errorMessage: "Required outputs missing: \(missingList)"
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
            succeeded: true,
            errorMessage: nil
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
}
