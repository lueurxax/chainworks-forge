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
    private let gooseExtensionRegistrySnapshotProvider: @Sendable () throws -> GooseExtensionRegistrySnapshot

    // MARK: - Init

    nonisolated init(
        transport: any GooseTransportProtocol,
        gooseExtensionRegistrySnapshotProvider: @escaping @Sendable () throws -> GooseExtensionRegistrySnapshot = {
            try GooseExtensionRegistryReader().snapshot()
        }
    ) {
        self.transport = transport
        self.gooseExtensionRegistrySnapshotProvider = gooseExtensionRegistrySnapshotProvider
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

        // Step 2: Resolve provider/model.
        // Frozen per-agent provider bindings are the canonical runtime authority for live runs.
        // App-scoped overrides are only a fallback when no frozen binding exists.
        let provider = context.providerBinding?.providerIdentifier ?? override?.provider ?? agent.provider
        let model = context.providerBinding?.model ?? override?.model ?? agent.model

        // Proposal 007 §7.7: Validate path boundaries before write-capable execution
        if agent.worktreeWriteEnabled, let worktreeRoot = context.workspace.worktreeRoot {
            try RepoSafetyGuard.validateWorktreeReady(worktreeRoot: worktreeRoot.path)
        }

        // Step 3: Create isolated session
        // REQ-005: Use worktree as working directory with write access for write-enabled agents.
        // For read-only repo-backed stages, prefer the frozen project root over the ephemeral
        // run workspace so proposal/review agents inspect the actual target repository.
        let useWorktree = agent.worktreeWriteEnabled && context.workspace.worktreeRoot != nil
        let readOnlyRoot = context.projectRoot?.path ?? context.workspace.workspaceRoot.path
        let workingDirectory = useWorktree
            ? context.workspace.worktreeRoot!.path
            : readOnlyRoot
        let mcpResolution = resolveMCPPolicy(agent: agent, context: context)
        if !mcpResolution.blockingIssues.isEmpty {
            throw GooseSessionBridgeError.mcpPolicyResolutionFailed(mcpResolution.blockingIssues.joined(separator: "; "))
        }

        let sessionRequest = GooseSessionRequest(
            systemPrompt: packet.systemPrompt,
            workingDirectory: workingDirectory,
            model: model,
            provider: provider,
            executionPolicy: GooseExecutionPolicy(
                permissionProfileID: agent.permissionProfile,
                workspaceMode: useWorktree ? "read_write" : "read_only",
                gitOperationsAllowed: useWorktree,
                releaseOperationsAllowed: false,
                repoWritesAllowed: useWorktree
            ),
            metadata: [
                "run_id": context.workspace.runID.uuidString,
                "stage_id": context.stageID,
                "agent_id": agent.id,
                "iteration": String(context.iteration),
                "attempt": String(context.attemptNumber)
            ],
            requestedExtensions: mcpResolution.predictedEffectiveRuntimeExtensionIDs
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
            actualEnabledExtensions: sessionResponse.actualEnabledExtensions,
            startupLatencyMilliseconds: sessionResponse.startupLatencyMilliseconds,
            eventStream: eventStream,
            transport: transport
        )
    }

    private func resolveMCPPolicy(
        agent: ResolvedAgent,
        context: ExecutionContext
    ) -> MCPPolicyResolutionReport {
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

        let gooseRegistry = try? gooseExtensionRegistrySnapshotProvider()
        return MCPPolicyResolver().resolve(
            agent: agent,
            catalog: catalog,
            providerBinding: context.providerBinding,
            gooseRegistry: gooseRegistry
        )
    }

    /// Submit a task prompt to an existing Goose session.
    func executeInExistingSession(
        sessionID: String,
        packet: ExecutionPacket
    ) async throws -> GooseSessionExecution {
        let promptRequest = GoosePromptRequest(
            content: packet.taskDirective,
            context: packet.contextAttachments
        )

        let eventStream = transport.submitPrompt(
            sessionID: sessionID,
            prompt: promptRequest
        )
        let runtimeState = try await transport.readSessionRuntimeState(sessionID: sessionID)

        return GooseSessionExecution(
            sessionID: sessionID,
            actualEnabledExtensions: runtimeState?.enabledExtensions,
            startupLatencyMilliseconds: nil,
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
        let handoffPacket = context.handoffPacket
        let lazyEvidenceToolAssets = handoffPacket.flatMap { handoff -> LazyEvidenceToolAssets? in
            guard !handoff.lazyArtifactRefs.isEmpty else { return nil }
            return ensureLazyEvidenceToolAssets(
                lazyArtifacts: handoff.lazyArtifactRefs,
                artifactRoot: context.workspace.artifactRoot
            )
        }

        // 1. System prompt
        let systemPrompt = buildSystemPrompt(
            agent: agent,
            expectedOutputs: expectedOutputs,
            catalog: context.catalog
        )

        // 2. Task directive
        let taskDirective = buildTaskDirective(
            agent: agent,
            task: task,
            context: context,
            expectedOutputs: expectedOutputs,
            catalog: context.catalog,
            lazyEvidenceToolAssets: lazyEvidenceToolAssets
        )

        // 3. Context attachments (input artifacts + workspace info)
        var attachments: [GooseContextAttachment] = []

        // Workspace context attachment
        let projectRootDescription = context.projectRoot?.path ?? "not provided"
        let worktreeRootDescription = context.workspace.worktreeRoot?.path ?? "not provisioned"
        let useWorktree = agent.worktreeWriteEnabled && context.workspace.worktreeRoot != nil
        let boundaryNote = useWorktree
            ? "IMPORTANT: This agent has write access to the worktree root. All file operations must use explicit absolute paths within the worktree root."
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
            Worktree Root: \(worktreeRootDescription)
            \(boundaryNote)
            """,
            path: nil
        ))

        // Input artifact attachments (strategy-aware handoff)
        if let handoff = handoffPacket {
            for (name, data) in handoff.mandatoryArtifacts.sorted(by: { $0.key < $1.key }) {
                let content = String(data: data, encoding: .utf8) ?? "<binary data, \(data.count) bytes>"
                attachments.append(GooseContextAttachment(
                    type: "artifact",
                    name: name,
                    content: content,
                    path: nil
                ))
            }

            for (name, summary) in handoff.summaries.sorted(by: { $0.key < $1.key }) {
                attachments.append(GooseContextAttachment(
                    type: "text",
                    name: "summary_\(name)",
                    content: summary,
                    path: nil
                ))
            }

            if let lazyEvidenceToolAssets {
                attachments.append(GooseContextAttachment(
                    type: "file",
                    name: "LazyEvidenceTool",
                    content: "Executable helper for on-demand lazy artifact retrieval.",
                    path: lazyEvidenceToolAssets.executablePath
                ))
                attachments.append(GooseContextAttachment(
                    type: "text",
                    name: "lazy_evidence_manifest",
                    content: buildLazyEvidenceToolManifest(handoff.lazyArtifactRefs),
                    path: nil
                ))
            }

            for (name, pointer) in handoff.lazyArtifactRefs.sorted(by: { $0.key < $1.key }) {
                if let path = pointer.absolutePath {
                    attachments.append(GooseContextAttachment(
                        type: "file",
                        name: "lazy_\(name)",
                        content: buildLazyArtifactAttachmentContent(pointer: pointer),
                        path: path
                    ))
                } else {
                    attachments.append(GooseContextAttachment(
                        type: "text",
                        name: "lazy_\(name)",
                        content: buildLazyArtifactAttachmentContent(pointer: pointer),
                        path: nil
                    ))
                }
            }

            attachments.append(GooseContextAttachment(
                type: "text",
                name: "strategy_fingerprint",
                content: handoff.fingerprintMaterial,
                path: nil
            ))
        } else {
            for (name, data) in context.inputArtifacts {
                let content = String(data: data, encoding: .utf8) ?? "<binary data, \(data.count) bytes>"
                attachments.append(GooseContextAttachment(
                    type: "artifact",
                    name: name,
                    content: content,
                    path: nil
                ))
            }
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
        catalog: AgentCatalog?
    ) -> String {
        var parts: [String] = []
        let structuredHints = structuredOutputHints(agent: agent, expectedOutputs: expectedOutputs, catalog: catalog)

        // Agent role
        parts.append("You are \(agent.title) (ID: \(agent.id)).")
        parts.append("Mode: \(agent.mode)")

        if let resolvedSkill = agent.resolvedSkill,
           !resolvedSkill.injectedContent.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            parts.append("")
            parts.append(resolvedSkill.injectedContent)
        }

        // Agent-specific prompt
        if !agent.prompt.isEmpty {
            parts.append("")
            parts.append("## Role and Instructions")
            parts.append(agent.prompt)
        }

        // Output contract
        if !structuredHints.isEmpty {
            parts.append("")
            parts.append("## Output Contracts")
            let contractList = Array(Set(structuredHints.map(\.contractID))).sorted().joined(separator: ", ")
            parts.append("You must produce outputs conforming to the structured contract(s): \(contractList)")
            parts.append("Required outputs for this task: \(expectedOutputs.joined(separator: ", "))")
        } else if let contract = agent.outputContract {
            parts.append("")
            parts.append("## Output Contract")
            parts.append("You must produce outputs conforming to contract: \(contract)")
            if !expectedOutputs.isEmpty {
                parts.append("Required outputs for this task: \(expectedOutputs.joined(separator: ", "))")
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
        return parts.joined(separator: "\n")
    }

    // MARK: - Private: Task Directive

    private static func buildTaskDirective(
        agent: ResolvedAgent,
        task: AgentTask,
        context: ExecutionContext,
        expectedOutputs: [String],
        catalog: AgentCatalog?,
        lazyEvidenceToolAssets: LazyEvidenceToolAssets?
    ) -> String {
        var parts: [String] = []
        let structuredHints = structuredOutputHints(agent: agent, expectedOutputs: expectedOutputs, catalog: catalog)

        parts.append("## Task: \(task.task)")
        parts.append("")

        // Input artifacts
        if let handoff = context.handoffPacket {
            if !handoff.mandatoryArtifacts.isEmpty || !handoff.summaries.isEmpty || !handoff.lazyArtifactRefs.isEmpty {
                parts.append("### Strategy Handoff")
                parts.append("Profile: \(handoff.profileID)")
                parts.append("Mode: \(handoff.mode.rawValue)")
                parts.append("Mandatory artifacts:")
                for name in handoff.mandatoryArtifacts.keys.sorted() { parts.append("- \(name)") }
                if !handoff.summaries.isEmpty {
                    parts.append("Summarized artifacts:")
                    for name in handoff.summaries.keys.sorted() { parts.append("- \(name)") }
                }
                if !handoff.lazyArtifactRefs.isEmpty {
                    parts.append("Lazy artifacts:")
                    for name in handoff.lazyArtifactRefs.keys.sorted() { parts.append("- \(name)") }
                    parts.append("Lazy artifacts are omitted from inline prompt context.")
                    if let lazyEvidenceToolAssets {
                        parts.append("Use the executable `get_lazy_artifact` helper attached as LazyEvidenceTool for canonical on-demand evidence retrieval.")
                        parts.append("Run: \(lazyEvidenceToolAssets.executablePath) <artifact_name>")
                    }
                    parts.append("Load lazy artifacts on demand from the attached file paths only when they become necessary.")
                }
                parts.append("")
            }
        } else if !context.inputArtifacts.isEmpty {
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
            parts.append("Before stopping, verify that every required output file exists on disk in the output directory and is non-empty.")
            parts.append("If any required output file is missing or empty, continue working and fix that before you stop.")
        }

        if task.task == "normalize_idea_and_open_run" {
            parts.append("")
            parts.append("### Task-Specific Guidance")
            parts.append("Use the `idea_body` attachment as the primary source of truth for normalization.")
            parts.append("Do not stop after analysis or narration alone. The task is incomplete until all three required files exist on disk.")
            parts.append("`idea_brief` must be a concise, structured normalized brief that captures the problem, desired outcome, scope, risks, and next workflow step.")
            parts.append("`run_state` must be machine-readable workflow state for the new run. Initialize loop counters, register approval checkpoints if needed, and point to the next stage.")
            parts.append("`orchestrator_summary` must be a human-readable summary of what was normalized, which decisions were made, and why the run should proceed.")
            parts.append("Prefer clear English output unless the source artifact explicitly requires another language.")
        }

        if !structuredHints.isEmpty {
            parts.append("")
            parts.append("### Structured Output Requirements")
            parts.append("CRITICAL: Each required output file must contain exactly one top-level JSON object and nothing else.")
            parts.append("Do not write markdown, tables, headings, fences, narrative prose, or companion files unless they are explicitly listed as required outputs.")
            parts.append("Do not wrap the JSON in code fences (``` or ```json). Write raw JSON only.")
            parts.append("If you want to explain the review, put that explanation inside JSON fields required by the contract.")
            for hint in structuredHints {
                parts.append("")
                parts.append("#### \(hint.outputName) -> \(hint.contractID)")
                for field in hint.requiredFields {
                    parts.append("- \(field)")
                }

                // For proposal_review contracts, provide an explicit JSON template
                // to prevent codex/gpt providers from producing invalid output
                if hint.contractID == "proposal_review_v1" {
                    parts.append("")
                    parts.append("Your output file MUST be valid JSON matching this exact structure:")
                    parts.append("""
                    {
                      "agent_id": "\(agent.id)",
                      "role": "\(agent.skillRole ?? agent.mode)",
                      "score": 7,
                      "decision": "approve",
                      "verdict": "The proposal is well-structured...",
                      "summary": "Brief summary of the review...",
                      "issues": ["issue 1", "issue 2"],
                      "blocking_issues": [],
                      "non_blocking_issues": ["minor concern 1"],
                      "suggestions": ["suggestion 1"],
                      "assumptions": ["assumption 1"]
                    }
                    """)
                    parts.append("Replace the example values with your actual review content. Keep the exact field names.")
                    parts.append("The \"agent_id\" field MUST be exactly \"\(agent.id)\".")
                    parts.append("The \"score\" field MUST be a number from 0 to 10.")
                    parts.append("The \"decision\" field MUST be one of: \"approve\", \"revise\", or \"reject\".")
                }
            }
        }

        // Stop condition
        parts.append("")
        parts.append("### Stop Condition")
        parts.append("Complete the task and produce all required output files. Do not continue beyond the task scope.")

        return parts.joined(separator: "\n")
    }

    private static func buildLazyArtifactAttachmentContent(pointer: ArtifactPointer) -> String {
        var lines: [String] = []
        lines.append("artifact_name: \(pointer.artifactName)")
        if let path = pointer.absolutePath {
            lines.append("absolute_path: \(path)")
        }
        lines.append("byte_count: \(pointer.byteCount)")
        return lines.joined(separator: "\n")
    }

    private static func buildLazyEvidenceToolManifest(_ lazyArtifacts: [String: ArtifactPointer]) -> String {
        struct LazyEvidenceToolManifest: Codable {
            struct ArtifactEntry: Codable {
                let path: String?
                let byteCount: Int
            }

            let toolName: String
            let owner: String
            let invocation: String
            let artifacts: [String: ArtifactEntry]
        }

        let manifest = LazyEvidenceToolManifest(
            toolName: "get_lazy_artifact",
            owner: "LazyEvidenceTool",
            invocation: "LazyEvidenceTool.get_lazy_artifact(\"artifact_name\")",
            artifacts: lazyArtifacts.mapValues { pointer in
                LazyEvidenceToolManifest.ArtifactEntry(path: pointer.absolutePath, byteCount: pointer.byteCount)
            }
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(manifest),
              let json = String(data: data, encoding: .utf8) else {
            return "{\"toolName\":\"get_lazy_artifact\",\"owner\":\"LazyEvidenceTool\"}"
        }
        return json
    }

    private static func ensureLazyEvidenceToolAssets(
        lazyArtifacts: [String: ArtifactPointer],
        artifactRoot: URL
    ) -> LazyEvidenceToolAssets? {
        let toolsDirectory = artifactRoot.appendingPathComponent(".lazy-evidence-tools", isDirectory: true)
        let manifestPath = toolsDirectory.appendingPathComponent("lazy_evidence_manifest.json")
        let executablePath = toolsDirectory.appendingPathComponent("get_lazy_artifact")

        do {
            try FileManager.default.createDirectory(
                at: toolsDirectory,
                withIntermediateDirectories: true,
                attributes: nil
            )
            try buildLazyEvidenceToolManifest(lazyArtifacts).write(to: manifestPath, atomically: true, encoding: .utf8)
            try buildLazyEvidenceToolScript().write(to: executablePath, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes(
                [.posixPermissions: NSNumber(value: 0o755)],
                ofItemAtPath: executablePath.path
            )
            return LazyEvidenceToolAssets(
                executablePath: executablePath.path,
                manifestPath: manifestPath.path
            )
        } catch {
            return nil
        }
    }

    private static func buildLazyEvidenceToolScript() -> String {
        """
        #!/usr/bin/env python3
        import json
        import pathlib
        import sys

        def main() -> int:
            if len(sys.argv) != 2:
                print("usage: get_lazy_artifact <artifact_name>", file=sys.stderr)
                return 64

            artifact_name = sys.argv[1]
            manifest_path = pathlib.Path(__file__).with_name("lazy_evidence_manifest.json")
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            entry = manifest.get("artifacts", {}).get(artifact_name)
            if not entry:
                print(f"lazy artifact not found: {artifact_name}", file=sys.stderr)
                return 66

            path = entry.get("path")
            if not path:
                print(f"lazy artifact path unavailable: {artifact_name}", file=sys.stderr)
                return 66

            sys.stdout.buffer.write(pathlib.Path(path).read_bytes())
            return 0

        if __name__ == "__main__":
            raise SystemExit(main())
        """
    }

    private struct LazyEvidenceToolAssets {
        let executablePath: String
        let manifestPath: String
    }

    private struct StructuredOutputHint {
        let outputName: String
        let contractID: String
        let requiredFields: [String]
    }

    private static func structuredOutputHints(
        agent: ResolvedAgent,
        expectedOutputs: [String],
        catalog: AgentCatalog?
    ) -> [StructuredOutputHint] {
        var hintsByOutput: [String: StructuredOutputHint] = [:]

        for outputName in expectedOutputs {
            if let catalog,
               let schema = OutputContractResolverV2.resolveSchema(
                    for: outputName,
                    agent: agent,
                    catalog: catalog
               ),
               schema.validationMode != .humanOnly {
                hintsByOutput[outputName] = StructuredOutputHint(
                    outputName: outputName,
                    contractID: schema.contractID,
                    requiredFields: schema.requiredFields
                )
                continue
            }

            if isProposalReviewOutput(outputName) {
                hintsByOutput[outputName] = StructuredOutputHint(
                    outputName: outputName,
                    contractID: ProposalReviewContractAdapter.reviewContractID,
                    requiredFields: reviewRequiredFields(agentID: agent.id)
                )
            } else if isProposalReviewSummaryOutput(outputName) {
                hintsByOutput[outputName] = StructuredOutputHint(
                    outputName: outputName,
                    contractID: ProposalReviewContractAdapter.summaryContractID,
                    requiredFields: reviewSummaryRequiredFields()
                )
            }
        }

        return hintsByOutput.values.sorted {
            if $0.outputName != $1.outputName {
                return $0.outputName < $1.outputName
            }
            return $0.contractID < $1.contractID
        }
    }

    private static func reviewRequiredFields(agentID: String) -> [String] {
        [
            "agent_id: String (use '\(agentID)')",
            "role: String",
            "score: Number (0-10)",
            "decision: String",
            "verdict: String",
            "summary: String",
            "issues: Array of Strings",
            "blocking_issues: Array of Strings",
            "non_blocking_issues: Array of Strings",
            "suggestions: Array of Strings",
            "assumptions: Array of Strings"
        ]
    }

    private static func reviewSummaryRequiredFields() -> [String] {
        [
            "pass: Boolean",
            "average_score: Number",
            "aggregate_score: Number",
            "min_individual_score: Number",
            "blocker_count: Integer",
            "summary: String",
            "required_changes: Array of Strings",
            "recurring_themes: Array of Strings",
            "decision: String"
        ]
    }

    private static func isProposalReviewOutput(_ outputName: String) -> Bool {
        [
            "proposal_review_po",
            "proposal_review_ux",
            "proposal_review_ui",
            "proposal_review_architect"
        ].contains(outputName)
    }

    private static func isProposalReviewSummaryOutput(_ outputName: String) -> Bool {
        outputName == "proposal_review_summary"
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
    let actualEnabledExtensions: [String]?
    let startupLatencyMilliseconds: Int?
    let eventStream: AsyncThrowingStream<GooseStreamEvent, Error>
    let transport: any GooseTransportProtocol

    /// Close the session after execution completes.
    func closeSession() async {
        // ARCH-027: Perform cleanup in a detached task.
        // If the main execution task was cancelled (e.g. by watchdog), the current 
        // async context is already marked as cancelled. A detached task ensures 
        // the DELETE request actually reaches the server regardless of the agent's state.
        Task.detached(priority: .background) {
            do {
                try await transport.closeSession(sessionID: sessionID)
            } catch {
                let msg = error.localizedDescription
                // 404 means the session was already closed by the backend (idle timeout or concurrent cleanup).
                // 'cancelled' is redundant here as we are already cleaning up.
                let isAlreadyClosed = msg.contains("404") || msg.contains("not found")
                let isCancelled = msg.contains("cancelled") || (error as? CancellationError) != nil

                if !isAlreadyClosed && !isCancelled {
                    print("Warning: Failed to cleanup Goose session \(sessionID): \(msg)")
                }
            }
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
    case mcpPolicyResolutionFailed(String)

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
        case .mcpPolicyResolutionFailed(let reason):
            return "Live execution blocked: MCP policy could not be honored. \(reason)"
        }
    }
}
