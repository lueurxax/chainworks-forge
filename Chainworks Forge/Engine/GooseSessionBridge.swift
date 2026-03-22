import Foundation

// MARK: - GooseSessionBridge (ARCH-027: one session per AgentExecution)

/// Creates an isolated Goose session for a single AgentExecution.
/// Binds workspace, prompt packet, and input artifacts into a structured execution request.
///
/// Invariants:
/// - Every live AgentExecution gets its own isolated Goose session (ARCH-027).
/// - No session reuse across agents or iterations.
/// - No reliance on session memory; state is reconstructed from artifacts (ARCH-030).
/// - Workspace is passed explicitly; no implicit cwd.
final class GooseSessionBridge: Sendable {

    // MARK: - Dependencies

    let transport: GooseTransport

    // MARK: - Init

    nonisolated init(transport: GooseTransport) {
        self.transport = transport
    }

    // MARK: - Session Lifecycle

    /// Create an isolated session for one agent execution and execute the task.
    /// Returns the session ID, the stream of events, and handles cleanup.
    func executeInIsolatedSession(
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext,
        override: LiveExecutionOverride?
    ) async throws -> GooseSessionExecution {
        // Step 1: Build the structured execution packet
        let packet = Self.buildExecutionPacket(agent: agent, task: task, context: context)

        // Step 2: Resolve provider/model (use override if present)
        let provider = override?.provider ?? agent.provider
        let model = override?.model ?? agent.model

        // Step 3: Create isolated session
        let sessionRequest = GooseSessionRequest(
            systemPrompt: packet.systemPrompt,
            workingDirectory: context.workspace.workspaceRoot.path,
            model: model,
            provider: provider,
            metadata: [
                "run_id": context.workspace.runID.uuidString,
                "stage_id": context.stageID,
                "agent_id": agent.id,
                "iteration": String(context.iteration),
                "attempt": String(context.attemptNumber)
            ]
        )

        let sessionResponse = try await transport.createSession(request: sessionRequest)

        // Step 4: Submit the task prompt and get streaming events
        let promptRequest = GoosePromptRequest(
            content: packet.taskDirective,
            context: packet.contextAttachments
        )

        let eventStream = transport.submitPrompt(
            sessionID: sessionResponse.sessionId,
            prompt: promptRequest
        )

        return GooseSessionExecution(
            sessionID: sessionResponse.sessionId,
            eventStream: eventStream,
            transport: transport
        )
    }

    // MARK: - Execution Packet Construction (Section 8.2)

    /// Build a structured execution packet for the agent.
    /// Packet sections: system prompt, run context, workspace context, input artifacts, task directive.
    static func buildExecutionPacket(
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext
    ) -> ExecutionPacket {
        // 1. System prompt
        let systemPrompt = buildSystemPrompt(agent: agent)

        // 2. Task directive
        let taskDirective = buildTaskDirective(agent: agent, task: task, context: context)

        // 3. Context attachments (input artifacts + workspace info)
        var attachments: [GooseContextAttachment] = []

        // Workspace context attachment
        attachments.append(GooseContextAttachment(
            type: "text",
            name: "workspace_context",
            content: """
            Run ID: \(context.workspace.runID.uuidString)
            Stage ID: \(context.stageID)
            Iteration: \(context.iteration)
            Attempt: \(context.attemptNumber)
            Workspace Root: \(context.workspace.workspaceRoot.path)
            Artifact Root: \(context.workspace.artifactRoot.path)
            IMPORTANT: No implicit working directory is allowed. All file operations must use explicit absolute paths within the workspace root.
            """,
            path: nil
        ))

        // Input artifact attachments
        for (name, data) in context.inputArtifacts {
            let content = String(data: data, encoding: .utf8) ?? "<binary data, \(data.count) bytes>"
            attachments.append(GooseContextAttachment(
                type: "artifact",
                name: name,
                content: content,
                path: nil
            ))
        }

        // Idea body attachment
        if !context.ideaBody.isEmpty {
            attachments.append(GooseContextAttachment(
                type: "text",
                name: "idea_body",
                content: context.ideaBody,
                path: nil
            ))
        }

        return ExecutionPacket(
            systemPrompt: systemPrompt,
            taskDirective: taskDirective,
            contextAttachments: attachments
        )
    }

    // MARK: - Private: System Prompt

    private static func buildSystemPrompt(agent: ResolvedAgent) -> String {
        var parts: [String] = []

        // Agent role
        parts.append("You are \(agent.title) (ID: \(agent.id)).")
        parts.append("Mode: \(agent.mode)")

        // Agent-specific prompt
        if !agent.prompt.isEmpty {
            parts.append("")
            parts.append("## Role and Instructions")
            parts.append(agent.prompt)
        }

        // Output contract
        if let contract = agent.outputContract {
            parts.append("")
            parts.append("## Output Contract")
            parts.append("You must produce outputs conforming to contract: \(contract)")
            parts.append("Required outputs: \(agent.outputs.joined(separator: ", "))")
        }

        // Boundaries
        parts.append("")
        parts.append("## Boundaries")
        parts.append("- You must write output files to the artifact output directory provided.")
        parts.append("- Do not perform any git operations.")
        parts.append("- Do not modify files outside the workspace root.")
        parts.append("- Do not rely on implicit working directory — use explicit paths.")

        return parts.joined(separator: "\n")
    }

    // MARK: - Private: Task Directive

    private static func buildTaskDirective(
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext
    ) -> String {
        var parts: [String] = []

        parts.append("## Task: \(task.task)")
        parts.append("")

        // Input artifacts
        if !context.inputArtifacts.isEmpty {
            parts.append("### Input Artifacts")
            for name in context.inputArtifacts.keys.sorted() {
                parts.append("- \(name)")
            }
            parts.append("")
        }

        // Expected outputs
        if !agent.outputs.isEmpty {
            parts.append("### Expected Outputs")
            parts.append("You MUST produce the following output files in the artifact directory:")
            for output in agent.outputs {
                parts.append("- \(output)")
            }
            parts.append("")
            parts.append("Output directory: \(context.workspace.artifactRoot.path)/\(context.stageID).\(context.iteration)/\(agent.id)/\(context.attemptNumber)/")
        }

        // Stop condition
        parts.append("")
        parts.append("### Stop Condition")
        parts.append("Complete the task and produce all required output files. Do not continue beyond the task scope.")

        return parts.joined(separator: "\n")
    }

    // MARK: - Validation

    /// Validate that the workspace is explicitly set and not implicit cwd.
    static func validateWorkspace(_ workspace: RunWorkspace) throws {
        let path = workspace.workspaceRoot.path
        guard !path.isEmpty else {
            throw GooseSessionBridgeError.implicitCWDRejected
        }
        guard path != FileManager.default.currentDirectoryPath else {
            throw GooseSessionBridgeError.implicitCWDRejected
        }
        guard path != "/" else {
            throw GooseSessionBridgeError.implicitCWDRejected
        }
    }
}

// MARK: - ExecutionPacket

/// Structured execution packet sent to the provider.
/// Contains everything the agent needs to execute a task.
struct ExecutionPacket: Sendable {
    let systemPrompt: String
    let taskDirective: String
    let contextAttachments: [GooseContextAttachment]
}

// MARK: - GooseSessionExecution

/// Represents an in-flight execution within an isolated Goose session.
struct GooseSessionExecution: Sendable {
    let sessionID: String
    let eventStream: AsyncThrowingStream<GooseStreamEvent, Error>
    let transport: GooseTransport

    /// Close the session after execution completes.
    func closeSession() async {
        do {
            try await transport.closeSession(sessionID: sessionID)
        } catch {
            // Log but don't fail — session may already be closed by the backend
            print("Warning: Failed to close Goose session \(sessionID): \(error.localizedDescription)")
        }
    }
}

// MARK: - LiveExecutionOverride (Section 9)

/// App-scoped override for the first live slice.
/// When enabled, all agents in the proposal-loop use the same provider/model/effort.
struct LiveExecutionOverride: Codable, Sendable {
    let enabled: Bool
    let provider: String
    let model: String
    let effort: String
}

// MARK: - GooseSessionBridgeError

enum GooseSessionBridgeError: Error, LocalizedError {
    case implicitCWDRejected
    case workspaceRootMissing
    case sessionCreationFailed(reason: String)

    var errorDescription: String? {
        switch self {
        case .implicitCWDRejected:
            return "Implicit working directory rejected — workspace must be explicit (ARCH-025)"
        case .workspaceRootMissing:
            return "Workspace root is not set"
        case .sessionCreationFailed(let reason):
            return "Goose session creation failed: \(reason)"
        }
    }
}
