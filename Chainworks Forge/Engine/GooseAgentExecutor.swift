import Foundation

// MARK: - RuntimeAgentExecutor (concrete AgentExecutor via Goose — Section 8.1)

/// Concrete implementation of `AgentExecutor` using a Goose backend.
/// Each execution creates an isolated session via `RuntimeSessionBridge` (ARCH-027).
final class RuntimeAgentExecutor: AgentExecutor, @unchecked Sendable {

    // MARK: - Dependencies

    /// Proposal 026: Per-agent transport factory. Resolves the correct transport
    /// for each agent based on its runtime profile / adapter family.
    let transportFactory: any RuntimeTransportFactory
    let override: LiveExecutionOverride?
    let sessionManager: AgentSessionManager?

    /// Callback for live execution events (for UI streaming).
    var onExecutionEvent: (@Sendable (String, ExecutionEvent) -> Void)?

    // MARK: - Init

    /// Proposal 026: Primary init with transport factory for per-agent transport resolution.
    init(
        transportFactory: any RuntimeTransportFactory,
        override: LiveExecutionOverride? = nil,
        sessionManager: AgentSessionManager? = nil
    ) {
        self.transportFactory = transportFactory
        self.override = override
        self.sessionManager = sessionManager
    }

    /// Convenience init wrapping a single transport (backward compat for tests).
    convenience init(
        transport: any RuntimeTransportProtocol,
        override: LiveExecutionOverride? = nil,
        sessionManager: AgentSessionManager? = nil
    ) {
        self.init(
            transportFactory: SingleTransportFactory(transport: transport),
            override: override,
            sessionManager: sessionManager
        )
    }

    /// Transport used for cancellation/cleanup of Goose sessions.
    var gooseTransportForCancellation: (any RuntimeTransportProtocol)? {
        (transportFactory as? DefaultRuntimeTransportFactory)?.gooseTransport
    }

    func prepareForAppTermination() {
        (transportFactory as? RuntimeTransportFactoryTerminationControlling)?
            .terminateActiveTransportsForAppShutdown()
    }

    /// Resolve the session bridge for a specific agent using the transport factory.
    private func sessionBridge(for agent: ResolvedAgent, binding: ResolvedProviderBinding?) throws -> RuntimeSessionBridge {
        let transport = try transportFactory.transport(for: agent, binding: binding)
        return RuntimeSessionBridge(transport: transport)
    }

    // MARK: - AgentExecutor Protocol

    private static let maxTransportRetries = 2
    private static let maxFreshSessionCollisionRetries = 2

    /// Maximum wall-clock time for a single agent execution attempt before the watchdog
    /// forcibly cancels it. Prevents runs from hanging for hours on stale sessions.
    static var executionTimeoutSeconds: TimeInterval = 1800 // 30 minutes
    static var acpProposalReviewStallSilenceSeconds: TimeInterval = 120
    static var acpProposalReviewReadLoopThreshold = 4
    static var acpProposalReviewStallPollIntervalMilliseconds: UInt64 = 5_000

    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult {
        var lastResult: AgentResult?
        for attempt in 0...Self.maxTransportRetries {
            let result = try await executeAttempt(
                task: task,
                agent: agent,
                context: context,
                attemptIndex: attempt
            )

            if result.succeeded || !isRetryableError(result) {
                return result
            }

            lastResult = result
            if attempt < Self.maxTransportRetries {
                try? await Task.sleep(for: .seconds(2))
            }
        }
        return lastResult!
    }

    private func isRetryableError(_ result: AgentResult) -> Bool {
        guard let error = result.errorMessage else { return false }
        if result.transportErrorKind == .timeout {
            return false
        }
        if error.contains("ACP proposal review stalled in read loop") {
            return false
        }

        let isTransportError = error.contains("timed out") || error.contains("-1001") || error.contains("timeout")
            || error.contains("Session not found") || error.contains("session not found")
            || error.contains("session became unavailable")
        let isContractViolation = error.contains("Required outputs missing") || error.contains("output contract")
            || error.contains("not valid JSON") || error.contains("Missing required field")

        // When outputs are missing but the agent's text reveals a provider-side session, limit,
        // or crash error, the failure is recoverable with a fresh session — override the
        // contract-violation classification.
        if isContractViolation, let text = result.accumulatedText {
            let lowered = text.lowercased()
            let isProviderSessionError = lowered.contains("error resuming session")
                || lowered.contains("invalid session identifier")
                || lowered.contains("session not found")
            let isProviderLimitError = isLimitExhaustionError(text)
            let isProviderCrash = lowered.contains("heap out of memory")
                || lowered.contains("fatal error")
                || lowered.contains("allocation failed")
                || lowered.contains("exit code")
            if isProviderSessionError || isProviderLimitError || isProviderCrash {
                return true
            }
            return false
        }

        return isTransportError
    }

    // MARK: - Execution Attempt Flow

    private func executeAttempt(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext,
        attemptIndex: Int
    ) async throws -> AgentResult {
        let startedAt = Date()
        let expectedOutputs = OutputContractResolverV2.expectedOutputs(for: task, agent: agent)

        try RuntimeSessionBridge.validateWorkspace(context.workspace)

        // 1. Resolve Session (Proposal 018)
        let sessionInfo: SessionResolutionInfo
        do {
            sessionInfo = try await resolveSession(task: task, agent: agent, context: context)
        } catch {
            return AgentResult.failure(
                "Session creation failed (attempt \(attemptIndex)): \(error.localizedDescription)",
                context: context,
                override: override,
                startedAt: startedAt
            )
        }

        // 2. Execute & Process Stream
        let eventBridge = ExecutionEventBridge()
        let agentID = agent.id
        let onEvent = onExecutionEvent
// 2. Stream & Process
let streamResult: ExecutionStreamResult
let sessionID = sessionInfo.execution.sessionID
ForgeLogger.session.info("[\(sessionID)] Stream Start. Agent: \(agent.id)")

do {
    let monitoredStream = monitoredEventStreamIfNeeded(
        sessionInfo.execution.eventStream,
        sessionInfo: sessionInfo,
        agent: agent
    )
    streamResult = try await processStreamWithSupervision(
        monitoredStream,
        agentID: agent.id,
        sessionID: sessionID,
        onEvent: { [agentID] event in
            // Structured event logging for visibility into "long-running" sessions
            switch event.type {
            case .toolCallStarted:
                if let tool = event.toolName {
                    ForgeLogger.session.debug("[\(sessionID)] Tool Active: \(agentID) -> \(tool)")
                }
            case .toolCallFinished:
                if let tool = event.toolName {
                    ForgeLogger.session.debug("[\(sessionID)] Tool Done: \(agentID) -> \(tool)")
                }
            case .error:
                ForgeLogger.session.error("[\(sessionID)] Stream Error: \(event.detail)")
            case .finish:
                ForgeLogger.session.info("[\(sessionID)] Stream Finish: \(event.detail)")
            default:
                break
            }
            onEvent?(agentID, event)
        },
        eventBridge: eventBridge
    )
} catch {
    ForgeLogger.session.error("[\(sessionID)] Stream Failed: \(error.localizedDescription)")
        return await handleStreamFailure(
            error: error,
            sessionInfo: sessionInfo,
            agent: agent,
        context: context,
        expectedOutputs: expectedOutputs,
        startedAt: startedAt,
        eventBridge: eventBridge
        )
}

let completedAt = Date()
await sessionInfo.execution.closeSession()
ForgeLogger.session.info("[\(sessionID)] Stream Success. Duration: \(Int(completedAt.timeIntervalSince(startedAt)))s")


        // 3. Update Economics & Budget (Proposal 018)
        let checkpoint = await updateEconomicsAndCheckBudget(
            streamResult: streamResult,
            sessionInfo: sessionInfo,
            agent: agent,
            context: context,
            startedAt: startedAt,
            completedAt: completedAt
        )

        // 4. Extract Outputs & Finalize
        let result = await finalizeSuccessResult(
            streamResult: streamResult,
            sessionInfo: sessionInfo,
            checkpoint: checkpoint,
            agent: agent,
            context: context,
            expectedOutputs: expectedOutputs,
            eventBridge: eventBridge,
            startedAt: startedAt,
            completedAt: completedAt
        )
        await settleCompletedGenerationIfNeeded(sessionInfo: sessionInfo, result: result)
        return result
    }

    private func processStreamWithSupervision(
        _ stream: AsyncThrowingStream<RuntimeStreamEvent, Error>,
        agentID: String,
        sessionID: String,
        onEvent: @Sendable @escaping (ExecutionEvent) -> Void,
        eventBridge: ExecutionEventBridge
    ) async throws -> ExecutionStreamResult {
        let timeoutSeconds = Self.executionTimeoutSeconds
        return try await withThrowingTaskGroup(of: ExecutionStreamResult.self) { group in
            group.addTask {
                try await eventBridge.processStream(stream, onEvent: onEvent)
            }
            group.addTask {
                try await Task.sleep(for: .seconds(timeoutSeconds))
                ForgeLogger.session.error("[\(sessionID)] Stream Watchdog Timeout after \(Int(timeoutSeconds))s")
                throw ExecutionError.timeout(agentID: agentID, seconds: Int(timeoutSeconds))
            }

            let result = try await group.next()!
            group.cancelAll()
            return result
        }
    }

    // MARK: - Internal Steps

    private struct SessionResolutionInfo {
        let execution: RuntimeSessionExecution
        let lineageID: UUID?
        let generationID: UUID?
        let reuseDisposition: SessionReuseDisposition
        let ownerKey: String
        let fingerprint: String
        let mcpResolution: MCPPolicyResolutionReport
    }

    private func resolveSession(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> SessionResolutionInfo {
        // Proposal 026: Resolve per-agent transport via factory
        let bridge = try sessionBridge(for: agent, binding: context.providerBinding)
        let packet = RuntimeSessionBridge.buildExecutionPacket(agent: agent, task: task, context: context)
        let (workingDirectory, workspaceMode) = resolveWorkingDirectoryAndMode(agent: agent, context: context)
        let fingerprint = calculateFingerprint(agent: agent, context: context, systemPrompt: packet.systemPrompt, workingDirectory: workingDirectory, workspaceMode: workspaceMode)
        let mcpResolution = resolveMCPPolicy(agent: agent, context: context, bridge: bridge)
        
        let ownerKey = InvocationOwnerKeyBuilder.build(
            runID: context.workspace.runID,
            agentID: agent.id,
            stageLineageID: context.stageLineageID ?? context.stageID,
            taskName: task.task,
            ownerExecutionLineageID: context.ownerExecutionLineageID
        )

        guard let sessionManager = sessionManager else {
            let execution = try await bridge.executeInIsolatedSession(agent: agent, task: task, context: context, override: override)
            return SessionResolutionInfo(
                execution: execution,
                lineageID: nil,
                generationID: nil,
                reuseDisposition: .fresh,
                ownerKey: ownerKey,
                fingerprint: fingerprint,
                mcpResolution: mcpResolution
            )
        }

        let lid = try await sessionManager.getOrCreateLineage(
            runID: context.workspace.runID,
            agentID: agent.id,
            scope: agent.sessionReuseScope,
            familyID: agent.sessionFamilyID
        )
        
        let lineage = try await sessionManager.getLineage(id: lid)
        // ARCH-001: Pass real recovery-branch truth (§4.1.1, §6.6).
        // ownerExecutionLineageID is imported from execution truth and ties reuse to one branch.
        let decision = SessionReusePolicy.evaluate(
            lineage: lineage,
            currentInvocationOwnerKey: ownerKey,
            currentBindingFingerprint: fingerprint,
            currentRecoveryBranchID: context.ownerExecutionLineageID
        )
        
        switch decision {
        case .reuse(let generation):
            ForgeLogger.session.debug("[\(lid)] Reuse session ID: \(generation.providerSessionID ?? "unknown") (Owner: \(ownerKey))")
            if let providerSessionID = generation.providerSessionID {
                let conflicts = try await sessionManager.providerSessionConflicts(
                    runID: context.workspace.runID,
                    providerSessionID: providerSessionID,
                    excludingLineageID: lid
                )
                if !conflicts.isEmpty {
                    let details = Self.describeProviderSessionConflicts(conflicts)
                    ForgeLogger.session.error("[\(lid)] Reuse denied for \(providerSessionID): \(details)")
                    let freshExecution = try await createFreshExecutionWithCollisionGuard(
                        agent: agent,
                        task: task,
                        context: context,
                        lineageID: lid,
                        ownerKey: ownerKey,
                        fingerprint: fingerprint,
                        workingDirectory: workingDirectory,
                        workspaceMode: workspaceMode,
                        bridge: bridge
                    )
                    let gid = try await sessionManager.createGeneration(
                        lineageID: lid,
                        invocationOwnerKey: ownerKey,
                        providerSessionID: freshExecution.sessionID,
                        bindingFingerprint: fingerprint,
                        workingDirectory: workingDirectory,
                        workspaceMode: workspaceMode,
                        runtimeProvider: resolvedRuntimeProvider(agent: agent, context: context),
                        runtimeModel: resolvedRuntimeModel(agent: agent, context: context)
                    )
                    try await sessionManager.recordEvent(lineageID: lid, generationID: gid, type: .created)
                    return SessionResolutionInfo(execution: freshExecution, lineageID: lid, generationID: gid, reuseDisposition: .fresh_session_required, ownerKey: ownerKey, fingerprint: fingerprint, mcpResolution: mcpResolution)
                }
            }
            // Attempt to reuse the existing provider session.
            // If the session has expired on the backend (e.g. idle timeout, server restart),
            // catch the "Session not found" error and fall back to a fresh session instead of failing.
            do {
                try await sessionManager.recordEvent(lineageID: lid, generationID: generation.id, type: .reused)
                let execution = try await bridge.executeInExistingSession(sessionID: generation.providerSessionID!, packet: packet)
                return SessionResolutionInfo(execution: execution, lineageID: lid, generationID: generation.id, reuseDisposition: .reused, ownerKey: ownerKey, fingerprint: fingerprint, mcpResolution: mcpResolution)
            } catch {
                let errorDesc = error.localizedDescription
                if isSessionMissingError(errorDesc) || errorDesc.contains("No active session") {
                    // Session expired — invalidate the stale generation and create a fresh session
                    ForgeLogger.session.info("[\(lid)] Expired session fallback: \(errorDesc)")
                    try? await sessionManager.invalidateGeneration(generationID: generation.id, reason: "Stale session: \(errorDesc)")
                    try? await sessionManager.recordEvent(lineageID: lid, generationID: generation.id, type: .invalidated)

                    let freshExecution = try await createFreshExecutionWithCollisionGuard(
                        agent: agent,
                        task: task,
                        context: context,
                        lineageID: lid,
                        ownerKey: ownerKey,
                        fingerprint: fingerprint,
                        workingDirectory: workingDirectory,
                        workspaceMode: workspaceMode,
                        bridge: bridge
                    )
                    let gid = try await sessionManager.createGeneration(
                        lineageID: lid,
                        invocationOwnerKey: ownerKey,
                        providerSessionID: freshExecution.sessionID,
                        bindingFingerprint: fingerprint,
                        workingDirectory: workingDirectory,
                        workspaceMode: workspaceMode,
                        runtimeProvider: resolvedRuntimeProvider(agent: agent, context: context),
                        runtimeModel: resolvedRuntimeModel(agent: agent, context: context)
                    )
                    try await sessionManager.recordEvent(lineageID: lid, generationID: gid, type: .created)
                    return SessionResolutionInfo(execution: freshExecution, lineageID: lid, generationID: gid, reuseDisposition: .fresh_after_transport_error, ownerKey: ownerKey, fingerprint: fingerprint, mcpResolution: mcpResolution)
                }
                throw error // Re-throw non-session errors
            }

        case .createFresh(let disposition, _):
            let execution = try await createFreshExecutionWithCollisionGuard(
                agent: agent,
                task: task,
                context: context,
                lineageID: lid,
                ownerKey: ownerKey,
                fingerprint: fingerprint,
                workingDirectory: workingDirectory,
                workspaceMode: workspaceMode,
                bridge: bridge
            )
            let gid = try await sessionManager.createGeneration(
                lineageID: lid,
                invocationOwnerKey: ownerKey,
                providerSessionID: execution.sessionID,
                bindingFingerprint: fingerprint,
                workingDirectory: workingDirectory,
                workspaceMode: workspaceMode,
                runtimeProvider: resolvedRuntimeProvider(agent: agent, context: context),
                runtimeModel: resolvedRuntimeModel(agent: agent, context: context)
            )
            try await sessionManager.recordEvent(lineageID: lid, generationID: gid, type: .created)
            return SessionResolutionInfo(execution: execution, lineageID: lid, generationID: gid, reuseDisposition: disposition, ownerKey: ownerKey, fingerprint: fingerprint, mcpResolution: mcpResolution)
            
        case .requireReset:
            let execution = try await createFreshExecutionWithCollisionGuard(
                agent: agent,
                task: task,
                context: context,
                lineageID: lid,
                ownerKey: ownerKey,
                fingerprint: fingerprint,
                workingDirectory: workingDirectory,
                workspaceMode: workspaceMode,
                bridge: bridge
            )
            let gid = try await sessionManager.createGeneration(
                lineageID: lid,
                invocationOwnerKey: ownerKey,
                providerSessionID: execution.sessionID,
                bindingFingerprint: fingerprint,
                workingDirectory: workingDirectory,
                workspaceMode: workspaceMode,
                runtimeProvider: resolvedRuntimeProvider(agent: agent, context: context),
                runtimeModel: resolvedRuntimeModel(agent: agent, context: context)
            )
            try await sessionManager.recordEvent(lineageID: lid, generationID: gid, type: .operator_reset)
            return SessionResolutionInfo(execution: execution, lineageID: lid, generationID: gid, reuseDisposition: .fresh_after_reset, ownerKey: ownerKey, fingerprint: fingerprint, mcpResolution: mcpResolution)
        }
    }

    private func resolveMCPPolicy(agent: ResolvedAgent, context: ExecutionContext, bridge: RuntimeSessionBridge) -> MCPPolicyResolutionReport {
        guard let catalog = context.catalog else {
            return agent.mcpProfileID == nil ? .none : MCPPolicyResolutionReport(
                profileID: agent.mcpProfileID ?? "none",
                requiredExtensions: [],
                optionalExtensions: [],
                requestedExtensions: [],
                requiredRuntimeExtensionIDs: [],
                optionalRuntimeExtensionIDs: [],
                predictedEffectiveExtensions: [],
                predictedEffectiveRuntimeExtensionIDs: [],
                deniedExtensions: [],
                warnings: [],
                blockingIssues: ["Catalog is unavailable; cannot resolve MCP profile for agent '\(agent.id)'."]
            )
        }

        // Dispatch registry provider by adapter family (Proposal 029).
        // - goose / nil: GooseExtensionRegistryReader (original path)
        // - codex_acp: CodexExtensionRegistryReader (Codex uses MCP natively)
        // - auggie_cli_acp, junie_cli_acp: nil (zero-MCP — no registry needed)
        let runtimeRegistry: RuntimeExtensionRegistrySnapshot?
        switch context.providerBinding?.adapterFamily {
        case "codex_acp":
            runtimeRegistry = try? CodexExtensionRegistryReader().snapshot()
        case "auggie_cli_acp", "junie_cli_acp":
            runtimeRegistry = nil
        default:
            runtimeRegistry = try? GooseExtensionRegistryReader().snapshot()
        }
        return MCPPolicyResolver().resolve(
            agent: agent,
            catalog: catalog,
            providerBinding: context.providerBinding,
            runtimeRegistry: runtimeRegistry,
            runtimeNamespaceOverride: bridge.transport.mcpRuntimeNamespace
        )
    }

    private func createFreshExecutionWithCollisionGuard(
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext,
        lineageID: UUID,
        ownerKey: String,
        fingerprint: String,
        workingDirectory: String,
        workspaceMode: String,
        bridge: RuntimeSessionBridge
    ) async throws -> RuntimeSessionExecution {
        guard let sessionManager else {
            return try await bridge.executeInIsolatedSession(agent: agent, task: task, context: context, override: override)
        }

        var lastCollisionDetails = ""

        for attempt in 0...Self.maxFreshSessionCollisionRetries {
            let execution: RuntimeSessionExecution
            do {
                execution = try await bridge.executeInIsolatedSession(
                    agent: agent,
                    task: task,
                    context: context,
                    override: override
                )
            } catch {
                let errorDesc = error.localizedDescription
                if isSessionMissingError(errorDesc) {
                    ForgeLogger.session.info("[\(lineageID)] Fresh session became unavailable before prompt submission; retrying fresh session. Error=\(errorDesc)")
                    if attempt == Self.maxFreshSessionCollisionRetries {
                        throw error
                    }
                    continue
                }
                throw error
            }

            let conflicts = try await sessionManager.providerSessionConflicts(
                runID: context.workspace.runID,
                providerSessionID: execution.sessionID,
                excludingLineageID: lineageID
            )

            if conflicts.isEmpty {
                return execution
            }

            lastCollisionDetails = Self.describeProviderSessionConflicts(conflicts)
            ForgeLogger.session.error("[\(lineageID)] Fresh session \(execution.sessionID) already belongs to another lineage. Owner=\(ownerKey) Fingerprint=\(fingerprint) WorkingDir=\(workingDirectory) Mode=\(workspaceMode) Details=\(lastCollisionDetails)")
            await execution.closeSession()

            if attempt == Self.maxFreshSessionCollisionRetries {
                break
            }
        }

        throw NSError(
            domain: "RuntimeAgentExecutor",
            code: 409,
            userInfo: [
                NSLocalizedDescriptionKey: "Provider session ID collision detected; refusing to attach session to the wrong lineage. \(lastCollisionDetails)"
            ]
        )
    }

    private func monitoredEventStreamIfNeeded(
        _ stream: AsyncThrowingStream<RuntimeStreamEvent, Error>,
        sessionInfo: SessionResolutionInfo,
        agent: ResolvedAgent
    ) -> AsyncThrowingStream<RuntimeStreamEvent, Error> {
        guard shouldMonitorACPProposalReviewStall(sessionInfo: sessionInfo, agent: agent) else {
            return stream
        }

        final class StallState: @unchecked Sendable {
            private let lock = NSLock()
            private var lastProgressAt = Date()
            private var readLoopStartCount = 0
            private var finished = false

            func recordProgress() {
                lock.lock()
                lastProgressAt = Date()
                readLoopStartCount = 0
                lock.unlock()
            }

            func recordReadLikeStart() {
                lock.lock()
                readLoopStartCount += 1
                lock.unlock()
            }

            func markFinished() {
                lock.lock()
                finished = true
                lock.unlock()
            }

            func snapshot() -> (lastProgressAt: Date, readLoopStartCount: Int, finished: Bool) {
                lock.lock()
                defer { lock.unlock() }
                return (lastProgressAt, readLoopStartCount, finished)
            }
        }

        let state = StallState()
        state.recordProgress()

        return AsyncThrowingStream { continuation in
            let producer = Task {
                do {
                    for try await event in stream {
                        switch event {
                        case .textChunk(let text):
                            if !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                state.recordProgress()
                            }
                        case .toolCallStarted(let toolName, _):
                            if isACPReadLoopTool(toolName) {
                                state.recordReadLikeStart()
                            } else {
                                state.recordProgress()
                            }
                        case .toolCallFinished(let toolName, _):
                            if !isACPReadLoopTool(toolName) {
                                state.recordProgress()
                            }
                        case .finalOutput, .finish, .sessionClosed:
                            state.markFinished()
                        default:
                            break
                        }

                        continuation.yield(event)

                        if case .sessionClosed = event {
                            continuation.finish()
                            return
                        }
                    }

                    state.markFinished()
                    continuation.finish()
                } catch {
                    state.markFinished()
                    continuation.finish(throwing: error)
                }
            }

            let monitor = Task {
                while !Task.isCancelled {
                    try? await Task.sleep(
                        nanoseconds: Self.acpProposalReviewStallPollIntervalMilliseconds * 1_000_000
                    )

                    let snapshot = state.snapshot()
                    if snapshot.finished {
                        return
                    }

                    let stalledFor = Date().timeIntervalSince(snapshot.lastProgressAt)
                    if snapshot.readLoopStartCount >= Self.acpProposalReviewReadLoopThreshold
                        && stalledFor >= Self.acpProposalReviewStallSilenceSeconds {
                        state.markFinished()
                        producer.cancel()
                        continuation.finish(throwing: ACPProposalReviewStallError(
                            silenceSeconds: stalledFor,
                            readLoopCount: snapshot.readLoopStartCount
                        ))
                        return
                    }
                }
            }

            continuation.onTermination = { @Sendable _ in
                producer.cancel()
                monitor.cancel()
            }
        }
    }

    private func shouldMonitorACPProposalReviewStall(
        sessionInfo: SessionResolutionInfo,
        agent: ResolvedAgent
    ) -> Bool {
        let runtimeNamespace = sessionInfo.execution.transport.mcpRuntimeNamespace?.lowercased()
        let isACP = runtimeNamespace != nil && runtimeNamespace != "goose"
        let reviewOutputs = Set([
            "proposal_review_po",
            "proposal_review_ux",
            "proposal_review_ui",
            "proposal_review_architect"
        ])
        let isProposalReview = agent.mode.hasPrefix("proposal_review.")
            || agent.outputs.contains(where: { reviewOutputs.contains($0) })
        return isACP && isProposalReview
    }

    private func isACPReadLoopTool(_ toolName: String) -> Bool {
        let normalized = toolName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return normalized == "read"
            || normalized == "read_file"
            || normalized == "read_workspace"
            || normalized == "permission:read"
    }

    private func handleStreamFailure(
        error: Error,
        sessionInfo: SessionResolutionInfo,
        agent: ResolvedAgent,
        context: ExecutionContext,
        expectedOutputs: [String],
        startedAt: Date,
        eventBridge: ExecutionEventBridge
    ) async -> AgentResult {
        await sessionInfo.execution.closeSession()
        let completedAt = Date()

        var salvaged = salvageOutputs(expectedOutputs: expectedOutputs, context: context, agent: agent)
        let returnedOutputs = parseReturnedOutputs(
            expectedOutputs: expectedOutputs,
            finalContent: nil,
            accumulatedText: eventBridge.accumulatedText
        )
        for (name, data) in returnedOutputs where salvaged[name] == nil {
            salvaged[name] = data
        }
        let outputPresence: OutputPresence = salvaged.isEmpty ? .none : .durableOutput
        let rawErrorMessage = error.localizedDescription
        let surfacedErrorMessage = surfacedStreamFailureMessage(
            rawErrorMessage: rawErrorMessage,
            reuseDisposition: sessionInfo.reuseDisposition
        )
        let transportKind = classifyTransportErrorKind(rawErrorMessage)
        let canonicalOutcome = classifyStreamFailureOutcome(errorMessage: rawErrorMessage, outputPresence: outputPresence)
        let sessionBecameUnavailableMidStream = isSessionMissingError(rawErrorMessage)

        var checkpoint: AgentSessionCheckpoint?
        if let gid = sessionInfo.generationID, let sessionManager = sessionManager {
            if sessionBecameUnavailableMidStream {
                try? await sessionManager.invalidateGeneration(
                    generationID: gid,
                    reason: "transport stream failure: \(rawErrorMessage)"
                )
                try? await sessionManager.recordEvent(
                    lineageID: sessionInfo.lineageID!,
                    generationID: gid,
                    type: .invalidated
                )
            } else if canonicalOutcome == .completedWithTransportError || canonicalOutcome == .timedOutAfterOutput {
                checkpoint = AgentSessionCheckpointBuilder.build(
                    executionResult: AgentResult.minimal(canonicalOutcome, sessionID: sessionInfo.execution.sessionID, duration: completedAt.timeIntervalSince(startedAt)),
                    eventLog: eventBridge.eventLog,
                    ownerKey: sessionInfo.ownerKey,
                    scope: agent.sessionReuseScope,
                    familyID: agent.sessionFamilyID
                )
                // DATA-001: Record checkpoint_created BEFORE invalidation (§6.4 rule 2)
                let checkpointData = try? JSONEncoder().encode(checkpoint)
                try? await sessionManager.recordCheckpointCreated(lineageID: sessionInfo.lineageID!, generationID: gid, checkpointData: checkpointData)
                try? await sessionManager.invalidateGeneration(generationID: gid, reason: "Stream failure: \(rawErrorMessage)")
                try? await sessionManager.recordEvent(lineageID: sessionInfo.lineageID!, generationID: gid, type: .invalidated)
            }
        }

        let failureMsg = failureMessage(for: canonicalOutcome, fallback: surfacedErrorMessage)
        salvaged = await ImplementationFailureArtifactSynthesizer.supplementMissingOutputs(
            existingOutputs: salvaged,
            expectedOutputs: expectedOutputs,
            agent: agent,
            context: context,
            failureSummary: failureMsg
        )
        let finalOutputPresence: OutputPresence = salvaged.isEmpty ? .none : .durableOutput
        let receiptArtifacts = ExecutionReceiptBuilder.buildReceipt(
            agentID: agent.id, sessionID: sessionInfo.execution.sessionID, stageID: context.stageID, iteration: context.iteration, attemptNumber: context.attemptNumber,
            startedAt: startedAt, completedAt: completedAt, events: eventBridge.eventLog, toolCalls: eventBridge.toolCalls, finalContent: nil,
            succeeded: canonicalOutcome == .completed, errorMessage: failureMsg, provider: resolvedRuntimeProvider(agent: agent, context: context), model: resolvedRuntimeModel(agent: agent, context: context), effort: resolvedRuntimeEffort(agent: agent, context: context),
            sessionReuseDisposition: sessionInfo.reuseDisposition.rawValue, sessionReuseScope: agent.sessionReuseScope.rawValue, sessionFamilyID: agent.sessionFamilyID
        )
        for (name, data) in receiptArtifacts { salvaged[name] = data }
        let lazyEvidenceArtifactHits = detectLazyEvidenceArtifactHits(
            toolCalls: eventBridge.toolCalls,
            handoffPacket: context.handoffPacket
        )
        let mcpServerMetrics = buildMCPServerMetrics(
            toolCalls: eventBridge.toolCalls,
            runtimeExtensionIDs: sessionInfo.execution.actualEnabledExtensions ?? sessionInfo.mcpResolution.predictedEffectiveRuntimeExtensionIDs
        )
        let runtimeTransport = context.providerBinding?.transport ?? "goose"

        return AgentResult(
            outputs: salvaged, logSnippet: "Stream failed but salvaged \(salvaged.count) artifacts. Error: \(failureMsg)", costCents: nil, succeeded: canonicalOutcome == .completed, errorMessage: failureMsg, sessionID: sessionInfo.execution.sessionID, durationSeconds: completedAt.timeIntervalSince(startedAt),
            providerReceipt: UsageReceiptNormalizer.makeReceipt(providerFamily: resolvedProviderFamily(agent: agent, context: context), configuredProviderID: context.providerBinding?.configuredProviderID, model: resolvedRuntimeModel(agent: agent, context: context), effort: resolvedRuntimeEffort(agent: agent, context: context), transport: runtimeTransport, costCents: nil, durationSeconds: completedAt.timeIntervalSince(startedAt), rawReceiptJSON: receiptArtifacts["\(agent.id)_receipt.json"]),
            resolvedModel: resolvedRuntimeModel(agent: agent, context: context), configuredProviderID: context.providerBinding?.configuredProviderID, adapterVersion: context.providerBinding?.adapterVersion,
            canonicalOutcome: canonicalOutcome, sessionLineageID: sessionInfo.lineageID, sessionGenerationID: sessionInfo.generationID, sessionReuseDisposition: sessionInfo.reuseDisposition, sessionCheckpoint: checkpoint, transportErrorKind: transportKind, outputPresence: finalOutputPresence, runtimeProvider: resolvedRuntimeProvider(agent: agent, context: context), runtimeModel: resolvedRuntimeModel(agent: agent, context: context),
            mcpProfileID: sessionInfo.mcpResolution.profileID,
            requestedMCPExtensions: sessionInfo.mcpResolution.requestedExtensions,
            effectiveMCPRuntimeExtensionIDs: sessionInfo.execution.actualEnabledExtensions ?? [],
            deniedMCPExtensions: sessionInfo.mcpResolution.deniedExtensions,
            mcpSessionStartupLatencyMilliseconds: sessionInfo.execution.startupLatencyMilliseconds,
            mcpServerMetrics: mcpServerMetrics,
            outcomeEnvelope: OutcomeEnvelope(canonicalOutcome: canonicalOutcome, transportErrorKind: transportKind, providerStopReason: nil, outputPresence: finalOutputPresence, rawErrorMessage: rawErrorMessage, rawFinishEvent: nil),
            lazyEvidenceArtifactHits: lazyEvidenceArtifactHits
        )
    }

    private func updateEconomicsAndCheckBudget(
        streamResult: ExecutionStreamResult,
        sessionInfo: SessionResolutionInfo,
        agent: ResolvedAgent,
        context: ExecutionContext,
        startedAt: Date,
        completedAt: Date
    ) async -> AgentSessionCheckpoint? {
        guard let sessionManager = sessionManager, let gid = sessionInfo.generationID else { return nil }

        let cost = estimateCost(streamResult: streamResult) ?? 0
        let tokens = Int64(streamResult.accumulatedText.count / 4)

        try? await sessionManager.updateGenerationUsage(
            generationID: gid, turnIncrement: 1, promptTokensIncrement: tokens, costCentsIncrement: cost, estimatedInputTokens: tokens
        )

        guard let lineage = try? await sessionManager.getLineage(id: sessionInfo.lineageID!),
              let generation = lineage.generations.first(where: { $0.id == gid }) else { return nil }

        // REQ-009: Build measured economic signals for ContextBudgetGuard (§6.3).
        let economicSignals = buildEconomicSignals(
            generation: generation,
            currentTurnTokens: tokens,
            sessionWasReused: sessionInfo.reuseDisposition == .reused || sessionInfo.reuseDisposition == .reused_after_resume
        )

        let budgetDecision = ContextBudgetGuard.evaluate(generation: generation, signals: economicSignals)
        switch budgetDecision {
        case .continueReuse:
            return nil
        case .compact(let reason), .invalidate(let reason):
            let type: AgentSessionEventType = if case .compact = budgetDecision { .compacted } else { .budget_exceeded }
            let checkpoint = AgentSessionCheckpointBuilder.build(
                executionResult: AgentResult.minimal(.completed, sessionID: sessionInfo.execution.sessionID, duration: completedAt.timeIntervalSince(startedAt)),
                eventLog: [],
                ownerKey: sessionInfo.ownerKey,
                scope: agent.sessionReuseScope,
                familyID: agent.sessionFamilyID
            )
            // Record checkpoint creation before invalidation (§6.4)
            let checkpointData = try? JSONEncoder().encode(checkpoint)
            try? await sessionManager.recordCheckpointCreated(lineageID: sessionInfo.lineageID!, generationID: gid, checkpointData: checkpointData)
            try? await sessionManager.invalidateGeneration(generationID: gid, reason: reason)
            try? await sessionManager.recordEvent(lineageID: sessionInfo.lineageID!, generationID: gid, type: type)
            return checkpoint
        }
    }

    /// REQ-009: Compute measured reuse-economics signals for ContextBudgetGuard (§6.3).
    private func buildEconomicSignals(
        generation: AgentSessionGeneration,
        currentTurnTokens: Int64,
        sessionWasReused: Bool
    ) -> ContextBudgetGuard.EconomicSignals {
        // Fresh-session baseline: average tokens per turn from this generation's history.
        let freshBaselineEstimate = max(1, generation.cumulativePromptTokens / max(1, Int64(generation.turnCount)))

        // Transcript growth ratio: current prompt size vs. fresh baseline.
        let transcriptGrowthRatio: Double = freshBaselineEstimate > 0
            ? Double(generation.estimatedInputTokens) / Double(freshBaselineEstimate)
            : 1.0

        // Cached token share: fraction of the prompt that's reused static prefix.
        let cachedTokenShare: Double
        if sessionWasReused && generation.estimatedInputTokens > 0 {
            let staticPrefix = max(0, generation.estimatedInputTokens - currentTurnTokens)
            cachedTokenShare = Double(staticPrefix) / Double(generation.estimatedInputTokens)
        } else {
            cachedTokenShare = 0.0
        }

        // Normalized savings: positive = reuse cheaper, negative = reuse more expensive.
        let freshCostCents = Double(freshBaselineEstimate) / 1000.0
        let reuseCostCents = Double(generation.estimatedInputTokens) / 1000.0 * (1.0 - cachedTokenShare * 0.5)
        let normalizedSavings = freshCostCents - reuseCostCents

        // Effective prompt size as fraction of a typical 200k context window.
        let contextWindowTokens: Double = 200_000
        let effectivePromptSizeFraction = Double(generation.estimatedInputTokens) / contextWindowTokens

        // Compaction churn: count how many times this generation has been compacted.
        // We approximate from the generation's lineage events if available.
        let compactionChurn = generation.lineage?.events
            .filter { $0.generationID == generation.id && $0.eventType == .compacted }
            .count ?? 0

        return ContextBudgetGuard.EconomicSignals(
            cachedTokenShare: cachedTokenShare,
            normalizedSavingsVersusFresh: normalizedSavings,
            transcriptGrowthRatio: transcriptGrowthRatio,
            effectivePromptSizeFraction: effectivePromptSizeFraction,
            compactionChurnCount: compactionChurn
        )
    }

    private func finalizeSuccessResult(
        streamResult: ExecutionStreamResult,
        sessionInfo: SessionResolutionInfo,
        checkpoint: AgentSessionCheckpoint?,
        agent: ResolvedAgent,
        context: ExecutionContext,
        expectedOutputs: [String],
        eventBridge: ExecutionEventBridge,
        startedAt: Date,
        completedAt: Date
    ) async -> AgentResult {
        var outputs = salvageOutputs(expectedOutputs: expectedOutputs, context: context, agent: agent)
        let returnedOutputs = parseReturnedOutputs(
            expectedOutputs: expectedOutputs,
            finalContent: streamResult.finalContent,
            accumulatedText: streamResult.accumulatedText
        )
        for (name, data) in returnedOutputs where outputs[name] == nil {
            outputs[name] = data
        }
        if outputs.isEmpty, let content = streamResult.finalContent, !content.isEmpty {
            if let primary = expectedOutputs.first { outputs[primary] = content.data(using: .utf8) ?? Data() }
        }

        let initialOutputPresence: OutputPresence = outputs.isEmpty ? .none : .durableOutput
        let canonicalOutcome = classifyCompletedStreamOutcome(
            outputPresence: initialOutputPresence,
            finishReason: streamResult.finishReason,
            hadExplicitFinalOutput: streamResult.finalContent != nil,
            accumulatedText: streamResult.accumulatedText
        )

        let initialMissingOutputs = expectedOutputs.filter { outputs[$0] == nil }
        let initialError: String? = if !initialMissingOutputs.isEmpty {
            "Required outputs missing: \(initialMissingOutputs.joined(separator: ", "))"
        } else if canonicalOutcome != .completed {
            failureMessage(for: canonicalOutcome, fallback: "Execution did not produce final output")
        } else {
            nil
        }

        outputs = await ImplementationFailureArtifactSynthesizer.supplementMissingOutputs(
            existingOutputs: outputs,
            expectedOutputs: expectedOutputs,
            agent: agent,
            context: context,
            failureSummary: initialError ?? "Execution stopped before producing all required implementation artifacts."
        )

        let outputPresence: OutputPresence = outputs.isEmpty ? .none : .durableOutput
        let missingOutputs = expectedOutputs.filter { outputs[$0] == nil }
        let finalOutcome: AgentCanonicalOutcome = !missingOutputs.isEmpty ? (canonicalOutcome == .completed ? .failedBeforeOutput : canonicalOutcome) : canonicalOutcome
        let finalError: String? = if !missingOutputs.isEmpty {
            "Required outputs missing: \(missingOutputs.joined(separator: ", "))"
        } else if finalOutcome != .completed {
            failureMessage(for: finalOutcome, fallback: "Execution did not produce final output")
        } else {
            nil
        }

        let receiptArtifacts = ExecutionReceiptBuilder.buildReceipt(
            agentID: agent.id, sessionID: sessionInfo.execution.sessionID, stageID: context.stageID, iteration: context.iteration, attemptNumber: context.attemptNumber,
            startedAt: startedAt, completedAt: completedAt, events: eventBridge.eventLog, toolCalls: eventBridge.toolCalls, finalContent: streamResult.finalContent,
            succeeded: finalOutcome == .completed, errorMessage: finalError, provider: resolvedRuntimeProvider(agent: agent, context: context), model: resolvedRuntimeModel(agent: agent, context: context), effort: resolvedRuntimeEffort(agent: agent, context: context),
            sessionReuseDisposition: sessionInfo.reuseDisposition.rawValue, sessionReuseScope: agent.sessionReuseScope.rawValue, sessionFamilyID: agent.sessionFamilyID
        )
        for (name, data) in receiptArtifacts { outputs[name] = data }
        let lazyEvidenceArtifactHits = detectLazyEvidenceArtifactHits(
            toolCalls: eventBridge.toolCalls,
            handoffPacket: context.handoffPacket
        )
        let mcpServerMetrics = buildMCPServerMetrics(
            toolCalls: eventBridge.toolCalls,
            runtimeExtensionIDs: sessionInfo.execution.actualEnabledExtensions ?? sessionInfo.mcpResolution.predictedEffectiveRuntimeExtensionIDs
        )

        let cost = estimateCost(streamResult: streamResult)
        let runtimeTransport = context.providerBinding?.transport ?? "goose"
        return AgentResult(
            outputs: outputs, logSnippet: buildLogSnippet(agent: agent, sessionID: sessionInfo.execution.sessionID, streamResult: streamResult, startedAt: startedAt, completedAt: completedAt),
            costCents: cost, succeeded: finalOutcome == .completed, errorMessage: finalOutcome == .completed ? nil : finalError, sessionID: sessionInfo.execution.sessionID, durationSeconds: completedAt.timeIntervalSince(startedAt),
            providerReceipt: UsageReceiptNormalizer.makeReceipt(providerFamily: resolvedProviderFamily(agent: agent, context: context), configuredProviderID: context.providerBinding?.configuredProviderID, model: resolvedRuntimeModel(agent: agent, context: context), effort: resolvedRuntimeEffort(agent: agent, context: context), transport: runtimeTransport, costCents: cost, durationSeconds: completedAt.timeIntervalSince(startedAt), rawReceiptJSON: receiptArtifacts["\(agent.id)_receipt.json"]),
            resolvedModel: resolvedRuntimeModel(agent: agent, context: context), configuredProviderID: context.providerBinding?.configuredProviderID, adapterVersion: context.providerBinding?.adapterVersion,
            canonicalOutcome: finalOutcome, sessionLineageID: sessionInfo.lineageID, sessionGenerationID: sessionInfo.generationID, sessionReuseDisposition: sessionInfo.reuseDisposition, sessionCheckpoint: checkpoint, transportErrorKind: nil, providerStopReason: streamResult.finishReason, outputPresence: outputPresence, runtimeProvider: resolvedRuntimeProvider(agent: agent, context: context), runtimeModel: resolvedRuntimeModel(agent: agent, context: context),
            mcpProfileID: sessionInfo.mcpResolution.profileID,
            requestedMCPExtensions: sessionInfo.mcpResolution.requestedExtensions,
            effectiveMCPRuntimeExtensionIDs: sessionInfo.execution.actualEnabledExtensions ?? [],
            deniedMCPExtensions: sessionInfo.mcpResolution.deniedExtensions,
            mcpSessionStartupLatencyMilliseconds: sessionInfo.execution.startupLatencyMilliseconds,
            mcpServerMetrics: mcpServerMetrics,
            accumulatedText: streamResult.accumulatedText,
            outcomeEnvelope: OutcomeEnvelope(canonicalOutcome: finalOutcome, transportErrorKind: nil, providerStopReason: streamResult.finishReason, outputPresence: outputPresence, rawErrorMessage: finalError, rawFinishEvent: streamResult.finishRaw),
            lazyEvidenceArtifactHits: lazyEvidenceArtifactHits
        )
    }

    private func settleCompletedGenerationIfNeeded(
        sessionInfo: SessionResolutionInfo,
        result: AgentResult
    ) async {
        guard let sessionManager, let generationID = sessionInfo.generationID else { return }

        let reason: String
        if result.succeeded {
            reason = "completed"
        } else if let errorMessage = result.errorMessage, !errorMessage.isEmpty {
            reason = errorMessage
        } else {
            reason = "execution_finished"
        }

        try? await sessionManager.closeGeneration(generationID: generationID, reason: reason)
        if let lineageID = sessionInfo.lineageID {
            try? await sessionManager.recordEvent(lineageID: lineageID, generationID: generationID, type: .closed)
        }
    }

    // MARK: - Private Helpers

    private func salvageOutputs(expectedOutputs: [String], context: ExecutionContext, agent: ResolvedAgent) -> [String: Data] {
        var salvaged: [String: Data] = [:]
        let outputDir = context.workspace.artifactRoot
            .appendingPathComponent("\(context.stageID).\(context.iteration)", isDirectory: true)
            .appendingPathComponent(agent.id, isDirectory: true)
            .appendingPathComponent("\(context.attemptNumber)", isDirectory: true)

        for outputName in expectedOutputs {
            let path = outputDir.appendingPathComponent(outputName)
            if FileManager.default.fileExists(atPath: path.path), let data = try? Data(contentsOf: path) {
                salvaged[outputName] = data
            }
        }
        return salvaged
    }

    private func parseReturnedOutputs(
        expectedOutputs: [String],
        finalContent: String?,
        accumulatedText: String
    ) -> [String: Data] {
        let sources = [finalContent, accumulatedText.isEmpty ? nil : accumulatedText].compactMap { $0 }
        var mergedOutputs: [String: Data] = [:]
        for source in sources {
            let parsed = parseReturnedOutputBlocks(from: source, expectedOutputs: expectedOutputs)
            for outputName in expectedOutputs where mergedOutputs[outputName] == nil {
                if let data = parsed[outputName] {
                    mergedOutputs[outputName] = data
                }
            }
        }
        return mergedOutputs
    }

    private func parseReturnedOutputBlocks(
        from content: String,
        expectedOutputs: [String]
    ) -> [String: Data] {
        let pattern = #"<<<CHAINWORKS_OUTPUT:([A-Za-z0-9._-]+)>>>\s*([\s\S]*?)\s*<<<END_CHAINWORKS_OUTPUT\s*>{2,3}"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return [:]
        }

        let expectedNames = Set(expectedOutputs)
        let nsRange = NSRange(content.startIndex..<content.endIndex, in: content)
        var outputs: [String: Data] = [:]

        for match in regex.matches(in: content, range: nsRange) {
            guard match.numberOfRanges == 3,
                  let nameRange = Range(match.range(at: 1), in: content),
                  let bodyRange = Range(match.range(at: 2), in: content) else {
                continue
            }

            let outputName = String(content[nameRange])
            guard expectedNames.contains(outputName) else { continue }

            let body = String(content[bodyRange]).trimmingCharacters(in: .newlines)
            guard !body.isEmpty else { continue }
            outputs[outputName] = Data(body.utf8)
        }

        return outputs
    }

    private func buildLogSnippet(agent: ResolvedAgent, sessionID: String, streamResult: ExecutionStreamResult, startedAt: Date, completedAt: Date) -> String {
        let duration = String(format: "%.1f", completedAt.timeIntervalSince(startedAt))
        return "Live execution of '\(agent.id)' completed in \(duration)s. Session: \(sessionID). Tool calls: \(streamResult.toolCalls.count)."
    }

    private func buildMCPServerMetrics(
        toolCalls: [ToolCallRecord],
        runtimeExtensionIDs: [String]
    ) -> [MCPServerExecutionMetric] {
        let allowedServerIDs = Set(runtimeExtensionIDs.map { $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() })
        guard !allowedServerIDs.isEmpty else { return [] }

        var aggregate: [String: (count: Int, requestBytes: Int64, responseBytes: Int64, promptDeltaBytes: Int64)] = [:]

        for toolCall in toolCalls {
            guard let serverID = resolveMCPServerID(for: toolCall.toolName, allowedServerIDs: allowedServerIDs) else {
                continue
            }
            let requestBytes = Int64(toolCall.rawPayload.lengthOfBytes(using: .utf8))
            let responseBytes = Int64((toolCall.responseRawPayload ?? "").lengthOfBytes(using: .utf8))
            var current = aggregate[serverID] ?? (count: 0, requestBytes: 0, responseBytes: 0, promptDeltaBytes: 0)
            current.count += 1
            current.requestBytes += requestBytes
            current.responseBytes += responseBytes
            current.promptDeltaBytes += responseBytes
            aggregate[serverID] = current
        }

        return aggregate.keys.sorted().map { serverID in
            let current = aggregate[serverID] ?? (count: 0, requestBytes: 0, responseBytes: 0, promptDeltaBytes: 0)
            return MCPServerExecutionMetric(
                serverID: serverID,
                toolCallCount: current.count,
                requestBytes: current.requestBytes,
                responseBytes: current.responseBytes,
                promptContextDeltaBytes: current.promptDeltaBytes
            )
        }
    }

    private func resolveMCPServerID(for toolName: String, allowedServerIDs: Set<String>) -> String? {
        let normalized = toolName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !normalized.isEmpty else { return nil }
        if let prefix = normalized.components(separatedBy: "__").first, allowedServerIDs.contains(prefix) {
            return prefix
        }
        if allowedServerIDs.contains(normalized) {
            return normalized
        }
        return nil
    }

    private func estimateCost(streamResult: ExecutionStreamResult) -> Int64? {
        let estimatedTokens = streamResult.accumulatedText.count / 4
        return max(1, Int64(estimatedTokens / 1000))
    }

    private func detectLazyEvidenceArtifactHits(
        toolCalls: [ToolCallRecord],
        handoffPacket: HandoffPacket?
    ) -> [String] {
        guard let handoffPacket, !handoffPacket.lazyArtifactRefs.isEmpty else { return [] }
        let lazyNames = Set(handoffPacket.lazyArtifactRefs.keys)
        let hits = lazyNames.filter { artifactName in
            toolCalls.contains { call in
                let haystack = "\(call.toolName)\n\(call.rawPayload)".lowercased()
                return haystack.contains("get_lazy_artifact") && haystack.contains(artifactName.lowercased())
            }
        }
        return hits.sorted()
    }

    private func classifyTransportErrorKind(_ errorMessage: String) -> TransportErrorKind {
        let lowercased = errorMessage.lowercased()
        if lowercased.contains("timed out") || lowercased.contains("timeout") || lowercased.contains("-1001") { return .timeout }
        if isLimitExhaustionError(errorMessage) { return .provider }
        if lowercased.contains("provider") { return .provider }
        if lowercased.contains("stream") { return .stream }
        return .unknown
    }

    private func classifyStreamFailureOutcome(errorMessage: String, outputPresence: OutputPresence) -> AgentCanonicalOutcome {
        if isLimitExhaustionError(errorMessage) {
            return outputPresence == .durableOutput ? .limitExhaustedAfterOutput : .limitExhaustedBeforeOutput
        }
        switch (classifyTransportErrorKind(errorMessage), outputPresence) {
        case (.timeout, .durableOutput): return .timedOutAfterOutput
        case (.timeout, .none): return .timedOutBeforeOutput
        case (_, .durableOutput): return .completedWithTransportError
        case (_, .none): return .failedBeforeOutput
        }
    }

    private func surfacedStreamFailureMessage(
        rawErrorMessage: String,
        reuseDisposition: SessionReuseDisposition
    ) -> String {
        if isCapacityExhaustionError(rawErrorMessage) {
            return "Provider capacity exhausted; retry the agent"
        }
        if isLimitExhaustionError(rawErrorMessage) {
            return "Provider or app limit exhausted"
        }
        if isSessionMissingError(rawErrorMessage) {
            if reuseDisposition == .reused || reuseDisposition == .reused_after_resume {
                return "Reused provider session became unavailable during execution"
            }
            return "Provider session became unavailable during execution"
        }
        return "Stream processing failed: \(rawErrorMessage)"
    }

    private static func describeProviderSessionConflicts(_ conflicts: [ProviderSessionConflict]) -> String {
        conflicts.map {
            "\($0.agentID) provider=\($0.runtimeProvider) model=\($0.runtimeModel) status=\($0.status.rawValue)"
        }
        .joined(separator: "; ")
    }

    private func classifyCompletedStreamOutcome(outputPresence: OutputPresence, finishReason: String?, hadExplicitFinalOutput: Bool, accumulatedText: String = "") -> AgentCanonicalOutcome {
        guard let finishReason else { return outputPresence == .durableOutput ? .completed : .failedBeforeOutput }
        if isLimitExhaustionReason(finishReason) { return outputPresence == .durableOutput ? .limitExhaustedAfterOutput : .limitExhaustedBeforeOutput }
        // Rate limit errors surfaced as agent text (not as stream errors) still indicate limit exhaustion.
        if outputPresence == .none, isLimitExhaustionError(accumulatedText) {
            return .limitExhaustedBeforeOutput
        }
        return (hadExplicitFinalOutput || outputPresence == .durableOutput) ? .completed : .failedBeforeOutput
    }

    private func isSessionMissingError(_ errorMessage: String) -> Bool {
        let lowercased = errorMessage.lowercased()
        return lowercased.contains("session not found")
            || lowercased.contains("failed to read session")
            || lowercased.contains("404")
            || (lowercased.contains("no active ") && lowercased.contains(" session"))
    }

    private func isLimitExhaustionReason(_ reason: String) -> Bool {
        let r = reason.lowercased()
        return r.contains("limit") || r.contains("quota") || r.contains("budget") || r.contains("max_tokens")
    }

    private func isCapacityExhaustionError(_ errorMessage: String) -> Bool {
        let lowercased = errorMessage.lowercased()
        return lowercased.contains("resource_exhausted")
            || lowercased.contains("model_capacity_exhausted")
            || lowercased.contains("capacity exhausted")
            || lowercased.contains("no capacity available")
    }

    private func isLimitExhaustionError(_ errorMessage: String) -> Bool {
        let lowercased = errorMessage.lowercased()
        return isCapacityExhaustionError(errorMessage)
            || lowercased.contains("rate limit")
            || lowercased.contains("quota")
            || lowercased.contains("credits")
            || lowercased.contains("credit balance")
            || lowercased.contains("usage limit")
            || lowercased.contains("limit exceeded")
            || lowercased.contains("budget exhausted")
            || lowercased.contains("max_tokens")
            || lowercased.contains("too many requests")
    }

    private func failureMessage(for outcome: AgentCanonicalOutcome, fallback: String) -> String {
        switch outcome {
        case .limitExhaustedBeforeOutput, .limitExhaustedAfterOutput:
            return fallback.isEmpty ? "Provider or app limit exhausted" : fallback
        case .timedOutBeforeOutput, .timedOutAfterOutput: return "Execution timed out"
        case .completedWithTransportError: return "Execution produced output but transport errored afterward"
        case .failedAfterOutputValidation: return "Output validation failed"
        case .cancelledBeforeOutput, .cancelledAfterOutput: return "Execution was cancelled"
        case .completed, .failedBeforeOutput: return fallback
        }
    }

    private func resolveWorkingDirectoryAndMode(agent: ResolvedAgent, context: ExecutionContext) -> (String, String) {
        let useWorktree = agent.worktreeWriteEnabled && context.workspace.worktreeRoot != nil
        let workingDirectory = useWorktree ? context.workspace.worktreeRoot!.path : (context.projectRoot?.path ?? context.workspace.workspaceRoot.path)
        return (workingDirectory, useWorktree ? "read_write" : "read_only")
    }

    private func calculateFingerprint(agent: ResolvedAgent, context: ExecutionContext, systemPrompt: String, workingDirectory: String, workspaceMode: String) -> String {
        BindingFingerprintBuilder.build(
            agent: agent,
            provider: resolvedRuntimeProvider(agent: agent, context: context),
            model: resolvedRuntimeModel(agent: agent, context: context),
            effort: resolvedRuntimeEffort(agent: agent, context: context),
            systemPrompt: systemPrompt,
            workingDirectory: workingDirectory,
            workspaceMode: workspaceMode,
            strategyFingerprintMaterial: context.handoffPacket?.fingerprintMaterial
        )
    }

    private func resolvedRuntimeProvider(agent: ResolvedAgent, context: ExecutionContext) -> String {
        context.providerBinding?.providerIdentifier ?? override?.provider ?? agent.provider
    }

    private func resolvedRuntimeModel(agent: ResolvedAgent, context: ExecutionContext) -> String {
        context.providerBinding?.model ?? override?.model ?? agent.model
    }

    private func resolvedRuntimeEffort(agent: ResolvedAgent, context: ExecutionContext) -> String {
        context.providerBinding?.effort ?? override?.effort ?? agent.effort
    }

    private func resolvedProviderFamily(agent: ResolvedAgent, context: ExecutionContext) -> String {
        if let family = context.providerBinding?.providerFamily {
            return family
        }
        if let provider = override?.provider, let family = ProviderFamily.from(runtimeIdentifier: provider) {
            return family.rawValue
        }
        return ProviderFamily.from(runtimeIdentifier: agent.provider)?.rawValue ?? agent.provider
    }
}

private struct ACPProposalReviewStallError: LocalizedError {
    let silenceSeconds: TimeInterval
    let readLoopCount: Int

    var errorDescription: String? {
        "ACP proposal review stalled in read loop without progress after \(Int(silenceSeconds))s (\(readLoopCount) read callbacks)"
    }
}

extension AgentResult {
    static func failure(_ msg: String, context: ExecutionContext, override: LiveExecutionOverride?, startedAt: Date) -> AgentResult {
        AgentResult(outputs: [:], logSnippet: msg, costCents: nil, succeeded: false, errorMessage: msg, sessionID: nil, durationSeconds: Date().timeIntervalSince(startedAt), providerReceipt: nil, resolvedModel: context.providerBinding?.model ?? override?.model, configuredProviderID: context.providerBinding?.configuredProviderID, adapterVersion: context.providerBinding?.adapterVersion)
    }

    static func minimal(_ outcome: AgentCanonicalOutcome, sessionID: String, duration: Double) -> AgentResult {
        AgentResult(outputs: [:], logSnippet: nil, costCents: nil, succeeded: outcome == .completed, errorMessage: nil, sessionID: sessionID, durationSeconds: duration, providerReceipt: nil, resolvedModel: nil, configuredProviderID: nil, adapterVersion: nil, canonicalOutcome: outcome)
    }
}
