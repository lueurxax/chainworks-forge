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

    /// Proposal 005: depends on `GooseTransportProtocol`, not concrete `GooseTransport`.
    let transport: any GooseTransportProtocol

    // MARK: - Init

    nonisolated init(transport: any GooseTransportProtocol) {
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

        // Proposal 007 §7.7: Validate path boundaries before write-capable execution
        let writableRoot = Self.resolveWritableRoot(agent: agent, context: context)
        if agent.worktreeWriteEnabled,
           let worktreeRoot = context.workspace.worktreeRoot,
           writableRoot?.standardizedFileURL == worktreeRoot.standardizedFileURL {
            try RepoSafetyGuard.validateWorktreeReady(worktreeRoot: worktreeRoot.path)
        }

        // Step 3: Create isolated session
        // REQ-005: Use worktree as working directory with write access for write-enabled agents.
        // For read-only repo-backed stages, prefer the frozen project root over the ephemeral
        // run workspace so proposal/review agents inspect the actual target repository.
        let useWritableRoot = agent.worktreeWriteEnabled && writableRoot != nil
        let useRepoWorktree = agent.worktreeWriteEnabled
            && context.workspace.worktreeRoot != nil
            && writableRoot?.standardizedFileURL == context.workspace.worktreeRoot?.standardizedFileURL
        let readOnlyRoot = context.projectRoot?.path ?? context.workspace.workspaceRoot.path
        let workingDirectory = useWritableRoot
            ? writableRoot!.path
            : readOnlyRoot

        let sessionRequest = GooseSessionRequest(
            systemPrompt: packet.systemPrompt,
            workingDirectory: workingDirectory,
            model: model,
            provider: provider,
            executionPolicy: GooseExecutionPolicy(
                permissionProfileID: agent.permissionProfile,
                workspaceMode: useWritableRoot ? "read_write" : "read_only",
                gitOperationsAllowed: useRepoWorktree,
                releaseOperationsAllowed: false,
                repoWritesAllowed: useRepoWorktree
            ),
            metadata: [
                "run_id": context.workspace.runID.uuidString,
                "stage_id": context.stageID,
                "agent_id": agent.id,
                "iteration": String(context.iteration),
                "attempt": String(context.attemptNumber)
            ]
        )

        let sessionResponse = try await transport.createSession(request: sessionRequest)
        guard sessionResponse.policyAcknowledgement?.accepted == true else {
            throw GooseSessionBridgeError.policyAcknowledgementMissing
        }

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
        // Proposal 013: V2 resolver — catalog-driven contract resolution
        let expectedOutputs = OutputContractResolverV2.expectedOutputs(for: task, agent: agent)
        let outputSchemas = expectedOutputs.compactMap { outputName -> (String, OutputContractSchemaV2)? in
            guard let schema = OutputContractResolverV2.resolveSchema(
                for: outputName,
                agent: agent,
                catalog: context.catalog
            ) else { return nil }
            return (outputName, schema)
        }

        // 1. System prompt
        let systemPrompt = buildSystemPrompt(
            agent: agent,
            expectedOutputs: expectedOutputs,
            outputSchemas: outputSchemas
        )

        // 2. Task directive
        let taskDirective = buildTaskDirective(
            agent: agent,
            task: task,
            context: context,
            expectedOutputs: expectedOutputs,
            outputSchemas: outputSchemas
        )

        // 3. Context attachments (input artifacts + workspace info)
        var attachments: [GooseContextAttachment] = []

        // Workspace context attachment
        let projectRootDescription = context.projectRoot?.path ?? "not provided"
        let writableRoot = resolveWritableRoot(agent: agent, context: context)
        let writableRootDescription = writableRoot?.path ?? "not provisioned"
        let useWritableRoot = agent.worktreeWriteEnabled && writableRoot != nil
        let usesRepoWorktree = agent.worktreeWriteEnabled
            && context.workspace.worktreeRoot != nil
            && writableRoot?.standardizedFileURL == context.workspace.worktreeRoot?.standardizedFileURL
        let boundaryNote = usesRepoWorktree
            ? "IMPORTANT: This agent has write access to the worktree root. All file operations must use explicit absolute paths within the worktree root."
            : useWritableRoot
                ? "IMPORTANT: This agent has write access to the writable root shown below. Use explicit absolute paths within that root for edits, while writing required outputs to their declared canonical paths."
                : context.projectRoot != nil
                ? "IMPORTANT: Treat the project root as the only source tree. Ignore any unexpected server cwd drift and use explicit absolute paths within the project root for reads, while writing outputs only into the artifact root."
                : "IMPORTANT: No implicit working directory is allowed. All file operations must use explicit absolute paths within the workspace root."
        attachments.append(GooseContextAttachment(
            type: "text",
            name: "workspace_context",
            content: """
            Run ID: \(context.workspace.runID.uuidString)
            Stage ID: \(context.stageID)
            Iteration: \(context.iteration)
            Attempt: \(context.attemptNumber)
            Workspace Root: \(context.workspace.workspaceRoot.path)
            Project Root: \(projectRootDescription)
            Artifact Root: \(context.workspace.artifactRoot.path)
            Writable Root: \(writableRootDescription)
            \(boundaryNote)
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

    private static func buildSystemPrompt(
        agent: ResolvedAgent,
        expectedOutputs: [String],
        outputSchemas: [(String, OutputContractSchemaV2)]
    ) -> String {
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
            if !expectedOutputs.isEmpty {
                parts.append("Required outputs for this task: \(expectedOutputs.joined(separator: ", "))")
            }
            if !outputSchemas.isEmpty {
                parts.append("Machine-readable primary artifacts are mandatory. Human-readable notes are optional only as separate companion files and never replace required outputs.")
            }
        }

        // Boundaries
        parts.append("")
        parts.append("## Boundaries")
        parts.append("- You must write output files to the artifact output directory provided.")
        parts.append("- Do not perform any git operations.")
        parts.append("- Do not rely on implicit working directory — use explicit absolute paths from the workspace context.")
        parts.append("- For read-only repo-backed stages, read source only from the Project Root provided in workspace_context.")
        parts.append("- If a writable worktree is provided, do not modify files outside that worktree root.")
        parts.append("- If the server cwd appears inconsistent with workspace_context, trust workspace_context and continue with explicit paths only.")
        if ProcessInfo.processInfo.environment["CHAINWORKS_DISABLE_XCODE_MCP"] == "1" {
            parts.append("- Do not call xcode_mcp or any IDE/editor MCP tools.")
            parts.append("- In tests, respond directly and complete the task without MCP tool discovery.")
        }

        return parts.joined(separator: "\n")
    }

    // MARK: - Private: Task Directive

    private static func buildTaskDirective(
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext,
        expectedOutputs: [String],
        outputSchemas: [(String, OutputContractSchemaV2)]
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
        if !expectedOutputs.isEmpty {
            parts.append("### Expected Outputs")
            parts.append("You MUST produce the following output files in the artifact directory:")
            for output in expectedOutputs {
                parts.append("- \(output)")
            }
            parts.append("")
            parts.append("Use the exact filenames listed above.")
            parts.append("Do not add file extensions like .md, .txt, or .json unless the filename above already includes that extension.")
            parts.append("Output directory: \(context.workspace.artifactRoot.path)/\(context.stageID).\(context.iteration)/\(agent.id)/\(context.attemptNumber)/")
            let canonicalOutputPaths = expectedOutputs.compactMap { output -> String? in
                guard let path = resolveCatalogArtifactPath(for: output, context: context) else { return nil }
                return "- \(output): \(path)"
            }
            if !canonicalOutputPaths.isEmpty {
                parts.append("")
                parts.append("### Canonical Output Paths")
                parts.append("If a canonical path is listed below, write the required output there. The runtime will collect it from that path.")
                canonicalOutputPaths.forEach { parts.append($0) }
            }
            if !outputSchemas.isEmpty {
                parts.append("")
                parts.append("### Machine Output Requirements")
                parts.append("Each required output file listed above is the primary contract artifact.")
                parts.append("Filename must be exactly the declared output name above.")
                parts.append("Do not add file extensions such as .md or .json unless the declared output name already includes that extension.")
                parts.append("Do not substitute markdown, prose, or alternate filenames for these files.")
                for (outputName, schema) in outputSchemas {
                    let fields = schema.requiredFields.joined(separator: ", ")
                    parts.append("- \(outputName): write a \(schema.machineFormat.rawValue.uppercased()) object")
                    if !fields.isEmpty {
                        parts.append("  Required fields: \(fields)")
                    }
                }
                parts.append("If you want to include a human-readable explanation, write it as an additional companion file after writing the required machine artifact.")
            }
        }

        if agent.outputContract == "proposal_review_v1" || agent.outputContract == "proposal_review_summary_v1" {
            parts.append("")
            parts.append("### Structured Output Requirements")
            parts.append("Each required output file must contain exactly one top-level JSON object and nothing else.")
            parts.append("Do not write markdown, tables, headings, fences, narrative prose, or companion files unless they are explicitly listed as required outputs.")
            parts.append("If you want to explain the review, put that explanation inside JSON fields required by the contract.")

            if agent.outputContract == "proposal_review_v1" {
                parts.append("")
                parts.append("#### Required Fields for proposal_review_v1:")
                parts.append("- agent_id: String (use '\(agent.id)')")
                parts.append("- role: String")
                parts.append("- score: Number (0-10)")
                parts.append("- decision: String")
                parts.append("- verdict: String")
                parts.append("- summary: String")
                parts.append("- issues: Array of Strings")
                parts.append("- blocking_issues: Array of Strings")
                parts.append("- non_blocking_issues: Array of Strings")
                parts.append("- suggestions: Array of Strings")
                parts.append("- assumptions: Array of Strings")
            } else if agent.outputContract == "proposal_review_summary_v1" {
                parts.append("")
                parts.append("#### Required Fields for proposal_review_summary_v1:")
                parts.append("- pass: Boolean")
                parts.append("- average_score: Number")
                parts.append("- aggregate_score: Number")
                parts.append("- min_individual_score: Number")
                parts.append("- blocker_count: Integer")
                parts.append("- summary: String")
                parts.append("- required_changes: Array of Strings")
                parts.append("- recurring_themes: Array of Strings")
                parts.append("- decision: String")
            }
        }

        // Stop condition
        parts.append("")
        parts.append("### Stop Condition")
        parts.append("Complete the task and produce all required output files. Do not continue beyond the task scope.")

        return parts.joined(separator: "\n")
    }

    private static func resolveWritableRoot(agent: ResolvedAgent, context: ExecutionContext) -> URL? {
        guard agent.worktreeWriteEnabled else { return nil }
        if let worktreeRoot = context.workspace.worktreeRoot {
            return worktreeRoot
        }
        guard let configuredPath = agent.worktreePath else {
            return nil
        }
        return PathTemplateResolver.resolvePath(configuredPath, projectRoot: context.projectRoot)
    }

    private static func resolveCatalogArtifactPath(for outputName: String, context: ExecutionContext) -> String? {
        guard let catalog = context.catalog,
              let hintedPath = catalog.artifacts[outputName] else {
            return nil
        }
        return PathTemplateResolver.resolvePath(hintedPath, projectRoot: context.projectRoot).path
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
/// Proposal 005: uses `GooseTransportProtocol` instead of concrete `GooseTransport`.
struct GooseSessionExecution: Sendable {
    let sessionID: String
    let eventStream: AsyncThrowingStream<GooseStreamEvent, Error>
    let transport: any GooseTransportProtocol

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
    case policyAcknowledgementMissing

    var errorDescription: String? {
        switch self {
        case .implicitCWDRejected:
            return "Implicit working directory rejected — workspace must be explicit (ARCH-025)"
        case .workspaceRootMissing:
            return "Workspace root is not set"
        case .sessionCreationFailed(let reason):
            return "Goose session creation failed: \(reason)"
        case .policyAcknowledgementMissing:
            return "Live execution blocked: backend did not acknowledge the required read-only execution policy"
        }
    }
}
