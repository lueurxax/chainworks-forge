import SwiftUI
import SwiftData

enum ArtifactInspectorSkillTruthFormatter {
    static func compactSummary(_ summary: String?) -> String? {
        guard let summary else { return nil }
        let normalized = summary
            .replacingOccurrences(of: "\r\n", with: "\n")
            .split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return nil }

        let maxLength = 220
        if normalized.count <= maxLength {
            return normalized
        }

        let cutoffIndex = normalized.index(normalized.startIndex, offsetBy: maxLength)
        return String(normalized[..<cutoffIndex]).trimmingCharacters(in: .whitespacesAndNewlines) + "…"
    }
}

enum ArtifactInspectorTraceabilityResolver {
    @MainActor
    static func downstreamConsumers(
        artifact: Artifact,
        run: Run,
        modelContext: ModelContext
    ) -> [AgentExecution] {
        let freshContext = ModelContext(modelContext.container)
        let stageSnapshots = RunStageSnapshotLoader.load(for: run.id, modelContext: freshContext)
        let allowedAgentExecutionIDs = Set(stageSnapshots.flatMap(\.agentExecutions).map(\.id))
        let artifactID = artifact.id

        let producingAgentExecutionID: UUID? = {
            let descriptor = FetchDescriptor<Artifact>(
                predicate: #Predicate<Artifact> { candidate in
                    candidate.id == artifactID
                }
            )
            let freshArtifact = try? freshContext.fetch(descriptor).first
            return freshArtifact?.agentExecution?.id
        }()

        let descriptor = FetchDescriptor<AgentExecution>()
        let runAgents = ((try? freshContext.fetch(descriptor)) ?? []).filter { agent in
            allowedAgentExecutionIDs.contains(agent.id)
        }

        return runAgents.filter { agent in
            guard agent.id != producingAgentExecutionID else { return false }

            if let bindingsData = agent.inputBindingsJSON,
               let bindings = try? JSONDecoder().decode([InputBinding].self, from: bindingsData) {
                return bindings.contains { $0.artifactName == artifact.name }
            }

            if let inputData = agent.consumedInputArtifactNamesJSON,
               let inputNames = try? JSONDecoder().decode([String].self, from: inputData) {
                return inputNames.contains(artifact.name)
            }

            return false
        }
    }
}

// MARK: - P005-OPS §9: Artifact Inspector V2

/// Upgraded artifact inspector with:
/// 1. Format-aware rendering (markdown, JSON, diff, generic text)
/// 2. Provenance chips (run, stage, agent, provider, model, effort, attempt, trust)
/// 3. Produced-by / consumed-by traceability
/// 4. Pin / unpin (affects reports and comparison)
/// 5. Open actions (reveal in Finder, open on disk, copy path)
///
/// Does NOT add repo-backed shortcuts (Proposal 007).
struct ArtifactInspectorView: View {
    let artifact: Artifact
    let run: Run

    @Environment(\.modelContext) private var modelContext
    @State private var content: String?
    @State private var isLoadingContent: Bool = false
    @State private var isPinned: Bool = false
    @State private var actionMessage: String?
    @State private var isSkillTruthExpanded = false
    @State private var isResolvedSkillContentExpanded = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Header
                HStack {
                    VStack(alignment: .leading) {
                        Text(artifact.name)
                            .font(.title2)
                            .accessibilityIdentifier("artifact-inspector-title")
                        Text(artifact.format.rawValue.uppercased())
                            .font(.caption)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(formatColor.opacity(0.15))
                            .foregroundStyle(formatColor)
                            .clipShape(Capsule())
                    }
                    Spacer()

                    // §9.4: Pin / unpin
                    Button {
                        togglePin()
                    } label: {
                        Image(systemName: isPinned ? "pin.fill" : "pin")
                            .foregroundStyle(isPinned ? .orange : .secondary)
                    }
                    .help(isPinned ? "Unpin artifact" : "Pin artifact")
                }

                if let actionMessage {
                    Text(actionMessage)
                        .font(.caption)
                        .foregroundStyle(DesignTokens.Status.error)
                }

                Divider()

                // §9.2: Provenance chips
                provenanceChips

                Divider()

                // §9.3: Produced-by / consumed-by
                traceabilitySection

                Divider()

                if shouldShowProposalLoopSummary, let summary = proposalLoopSummary {
                    GroupBox("Proposal-loop feedback summary (Proposal 022)") {
                        VStack(alignment: .leading, spacing: 4) {
                            let reviewCorpusLabel = summary.reviewCorpusBundlePresent
                                ? "present (\(summary.reviewCorpusRawArtifactCount.map(String.init) ?? "unknown") raw)"
                                : "missing"
                            LabeledContent("Review corpus bundle", value: reviewCorpusLabel)
                            LabeledContent("Backlog", value: "\(summary.backlogItemCount)")
                            LabeledContent("Unresolved", value: "\(summary.unresolvedItemCount)")
                            LabeledContent("Deferred", value: "\(summary.deferredItemCount)")
                            LabeledContent("Addressed", value: "\(summary.addressedItemCount)")
                            LabeledContent("Merge provenance", value: "\(summary.mergeProvenanceItemCount)")
                            LabeledContent("Coverage", value: summary.coverageStatusSummary)
                            if let growthRatio = summary.proposalGrowthRatio {
                                LabeledContent("Proposal growth", value: String(format: "%.2fx", growthRatio))
                            }
                            if let scoreDelta = summary.scoreDeltaSinceLastReview {
                                LabeledContent("Score delta", value: String(format: "%.2f", scoreDelta))
                            }
                            if let recommendation = summary.growthGuardRecommendation {
                                LabeledContent("Growth guard", value: recommendation)
                            }
                            if let nextAction = summary.boundedNextAction {
                                LabeledContent("Bounded next action", value: nextAction)
                            }
                            if let rationale = summary.targetedReviewerSummary {
                                Text("Targeted rereview")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                Text(rationale)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .padding(.leading, 8)
                            }
                        }
                    }

                    Divider()
                }

                // §9.1: Format-aware rendering
                if isLoadingContent {
                    ProgressView("Loading artifact…")
                        .frame(maxWidth: .infinity, minHeight: 160, alignment: .center)
                } else if let content {
                    ArtifactContentRenderer(
                        content: content,
                        context: .artifactBacked(artifact: artifact, run: run)
                    )
                } else {
                    ContentUnavailableView(
                        "Content Unavailable",
                        systemImage: "doc.questionmark",
                        description: Text("Could not load artifact from disk.")
                    )
                }

                Divider()

                if shouldShowSkillTruthSection {
                    skillTruthSection
                    Divider()
                }

                // §9.5: Open actions
                openActions
            }
            .padding()
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, minHeight: 560, alignment: .topLeading)
        .accessibilityIdentifier("artifact-inspector-view")
        .navigationTitle("Artifact Inspector")
        .task(id: artifact.id) {
            isLoadingContent = true
            content = await Self.loadArtifactContent(from: artifact.filePath)
            isLoadingContent = false
            isPinned = artifact.isPinned
        }
    }

    // MARK: - Provenance Chips (§9.2)

    private var provenanceChips: some View {
        FlowLayout(spacing: 6) {
            provenanceChip("Run", value: run.id.uuidString.prefix(8), icon: "play.circle")
            provenanceChip("Stage", value: artifact.stageID, icon: "rectangle.stack")
            provenanceChip("Agent", value: artifact.agentID, icon: "person.circle")
            provenanceChip("Provider", value: artifact.provider, icon: "server.rack")
            if let idea = run.idea {
                provenanceChip("Parent Idea", value: idea.isArchived ? "archived" : "active", icon: idea.isArchived ? "archivebox.fill" : "lightbulb.fill")
            }
            if let model = artifact.model {
                provenanceChip("Model", value: model, icon: "cpu")
            }
            if let effort = artifact.effort {
                provenanceChip("Effort", value: effort, icon: "gauge.medium")
            }
            provenanceChip("Attempt", value: "#\(artifact.attemptNumber)", icon: "arrow.clockwise")
            provenanceChip("Trust", value: run.runtimeTrustDisplayLabel, icon: "shield")
        }
    }

    private func provenanceChip(_ label: String, value: some StringProtocol, icon: String) -> some View {
        HStack(spacing: 3) {
            Image(systemName: icon)
                .font(.caption2)
            Text(verbatim: "\(label): \(value)")
                .font(.caption2)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Color.secondary.opacity(0.1))
        .clipShape(Capsule())
    }

    // MARK: - Traceability (§9.3)

    private var traceabilitySection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Traceability")
                .font(.caption.bold())

            // Produced-by
            if let agentExec = artifact.agentExecution {
                HStack {
                    Image(systemName: "arrow.right.circle")
                        .foregroundStyle(.green)
                    Text("Produced by")
                        .font(.caption)
                    Text("\(agentExec.agentTitle) (\(agentExec.taskName))")
                        .font(.caption.monospaced())
                }
            }

            // Downstream consumers — find agents that consumed THIS artifact
            let consumers = downstreamConsumers
            if !consumers.isEmpty {
                ForEach(consumers, id: \.id) { consumer in
                    HStack {
                        Image(systemName: "arrow.left.circle")
                            .foregroundStyle(.blue)
                        Text("Consumed by")
                            .font(.caption)
                        Text("\(consumer.agentTitle) (\(consumer.taskName))")
                            .font(.caption.monospaced())
                    }
                }
            } else {
                HStack {
                    Image(systemName: "circle.dashed")
                        .foregroundStyle(.secondary)
                    Text("No downstream consumers found")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    @ViewBuilder
    private var skillTruthSection: some View {
        if let agentExecution = artifact.agentExecution {
            let resolvedSkills = decodeResolvedSkills(from: run)
            let resolvedSkill = agentExecution.skillRef.flatMap { resolvedSkills[$0] }

            DisclosureGroup(isExpanded: $isSkillTruthExpanded) {
                VStack(alignment: .leading, spacing: 8) {
                    if let skillRef = agentExecution.skillRef {
                        LabeledContent("Skill Ref", value: skillRef)
                    }
                    if let skillType = agentExecution.skillType {
                        LabeledContent("Skill Type", value: skillType)
                    }
                    if let skillRole = agentExecution.skillRole {
                        LabeledContent("Skill Role", value: skillRole)
                    }
                    if let hash = agentExecution.skillSnapshotHash {
                        LabeledContent("Injected Skill Hash", value: hash)
                    }
                    if let summary = ArtifactInspectorSkillTruthFormatter.compactSummary(agentExecution.skillContentSummary) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Skill Summary")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text(summary)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                    if let resolvedSkill {
                        DisclosureGroup(isExpanded: $isResolvedSkillContentExpanded) {
                            ArtifactContentRenderer(
                                content: resolvedSkill.resolvedContent,
                                context: .explicit(format: .markdown)
                            )
                            .padding(.top, 6)
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Resolved Skill Content")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                Text("Open only when you need to inspect injected skill instructions.")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
                .padding(.top, 8)
            } label: {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Skill Truth")
                        .font(.headline)
                    Text("Related diagnostic metadata for the producing agent. Kept secondary so the artifact body stays primary.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .accessibilityIdentifier("artifact-inspector-skill-truth")
        }
    }

    private var proposalLoopSummary: ProposalLoopFeedbackSummary? {
        let runID = run.id
        let descriptor = FetchDescriptor<Artifact>(
            predicate: #Predicate<Artifact> { candidate in
                candidate.runID == runID
            }
        )
        let allArtifacts = (try? modelContext.fetch(descriptor)) ?? []
        let relevantNames: Set<String> = [
            "score_lift_backlog",
            "proposal_feedback_coverage",
            "proposal_review_summary"
        ]
        let relevantArtifacts = allArtifacts.filter { relevantNames.contains($0.name) }
        return ProposalLoopFeedbackParser.parseSummary(from: relevantArtifacts)
    }

    /// P005-OPS §9.3: Find downstream consumers via inputBindingsJSON (preferred)
    /// with consumedInputArtifactNamesJSON as fallback.
    private var downstreamConsumers: [AgentExecution] {
        ArtifactInspectorTraceabilityResolver.downstreamConsumers(
            artifact: artifact,
            run: run,
            modelContext: modelContext
        )
    }

    private var shouldShowProposalLoopSummary: Bool {
        artifact.name == "proposal_review_summary"
        || artifact.name == "score_lift_backlog"
        || artifact.name == "proposal_feedback_coverage"
    }

    private var shouldShowSkillTruthSection: Bool {
        artifact.agentExecution?.skillRef != nil
            || artifact.agentExecution?.skillType != nil
            || artifact.agentExecution?.skillRole != nil
            || artifact.agentExecution?.skillSnapshotHash != nil
    }

    private func decodeResolvedSkills(from run: Run) -> [String: ResolvedSkill] {
        guard let data = run.resolvedSkillsJSON else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedSkill].self, from: data)) ?? [:]
    }

    // MARK: - Open Actions (§9.5)

    private var openActions: some View {
        HStack(spacing: 12) {
            Button("Reveal in Finder", systemImage: "folder") {
                let url = URL(fileURLWithPath: artifact.filePath)
                NSWorkspace.shared.activateFileViewerSelecting([url])
            }

            Button("Open on Disk", systemImage: "doc.text") {
                NSWorkspace.shared.open(URL(fileURLWithPath: artifact.filePath))
            }

            Button("Copy Path", systemImage: "doc.on.clipboard") {
                ArtifactPathClipboard.copy(path: artifact.filePath)
            }
        }
        .buttonStyle(.bordered)
    }

    // MARK: - Pin / Unpin (§9.4)

    private func togglePin() {
        let previousPinned = isPinned
        let updatedPinned = !isPinned
        isPinned = updatedPinned
        artifact.isPinned = updatedPinned
        do {
            try modelContext.save()
            actionMessage = nil
        } catch {
            isPinned = previousPinned
            artifact.isPinned = previousPinned
            ForgeLogger.ui.error("Failed to update pin state for artifact \(artifact.id): \(error.localizedDescription)")
            actionMessage = "Failed to update pin state: \(error.localizedDescription)"
        }
    }

    private var formatColor: Color {
        switch artifact.format {
        case .markdown: return .blue
        case .json: return .orange
        case .diff: return .green
        case .report: return .purple
        }
    }

    private static func loadArtifactContent(from path: String) async -> String? {
        await Task.detached(priority: .userInitiated) {
            try? SecurityScopedAccess.loadString(from: URL(fileURLWithPath: path))
        }.value
    }
}

// MARK: - FlowLayout Helper

/// Simple flow layout for provenance chips.
struct FlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = layout(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = layout(proposal: proposal, subviews: subviews)
        for (index, position) in result.positions.enumerated() {
            subviews[index].place(
                at: CGPoint(x: bounds.minX + position.x, y: bounds.minY + position.y),
                proposal: .unspecified
            )
        }
    }

    private struct LayoutResult {
        let size: CGSize
        let positions: [CGPoint]
    }

    private func layout(proposal: ProposedViewSize, subviews: Subviews) -> LayoutResult {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > maxWidth && x > 0 {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            positions.append(CGPoint(x: x, y: y))
            rowHeight = max(rowHeight, size.height)
            x += size.width + spacing
        }

        return LayoutResult(
            size: CGSize(width: maxWidth, height: y + rowHeight),
            positions: positions
        )
    }
}

#Preview("Artifact Inspector — Skill Truth Secondary") {
    @MainActor
    func seededContainer() -> ModelContainer {
        PreviewSupport.makeModelContainer { context in
            let idea = Idea(title: "Preview Idea", body: "Artifact inspector preview")
            context.insert(idea)

            let root = FileManager.default.temporaryDirectory
                .appendingPathComponent("ArtifactInspectorPreview", isDirectory: true)
            let artifactsRoot = root.appendingPathComponent("artifacts", isDirectory: true)
            try? FileManager.default.createDirectory(at: artifactsRoot, withIntermediateDirectories: true)

            let artifactURL = artifactsRoot.appendingPathComponent("proposal_review_architect.md")
            try? """
            # Review Triad

            This is the primary artifact body and it should stay visually dominant.

            - The document content must appear before related skill metadata.
            - Related diagnostic payloads should be collapsed by default.

            ## Notes

            Long-form document rendering should remain readable.
            """.write(to: artifactURL, atomically: true, encoding: .utf8)

            let repo = RunRepository(context: context)
            let run = try! repo.createRun(
                for: idea,
                workflowID: "proposal_loop_live",
                workflowTitle: "Proposal Loop (Live)",
                workflowSnapshotHash: "preview-workflow",
                catalogSnapshotHash: "preview-catalog",
                workflowSourcePath: "preview/workflow.yaml",
                catalogSourcePath: "preview/agents.yaml",
                workflowSnapshotJSON: Data(),
                catalogSnapshotJSON: Data(),
                workspaceRoot: root.path,
                artifactRoot: artifactsRoot.path,
                planCompilerVersion: 1
            )

            let stage = StageExecution(stageID: "state_4_proposal_reviewed", label: "Proposal reviewed", status: .completed)
            stage.run = run
            context.insert(stage)

            let agentExecution = AgentExecution(
                agentID: "proposal_reviewer_architect",
                agentTitle: "Architect Reviewer",
                taskName: "Review proposal",
                status: .completed,
                provider: "codex",
                effort: "high"
            )
            agentExecution.stageExecution = stage
            agentExecution.skillRef = "proposal_review_triad"
            agentExecution.skillType = SkillType.external.rawValue
            agentExecution.skillRole = "architecture-only"
            agentExecution.skillSnapshotHash = "preview-skill-hash"
            agentExecution.skillContentSummary = """
            ---
            name: proposal-review-triad

            description: Review repo-local proposals with evidence-first architecture critique.

            Use architecture-only mode.
            """

            let resolvedSkill = ResolvedSkill(
                id: "proposal_review_triad",
                type: .external,
                resolvedContent: """
                # Proposal Review Triad

                This diagnostic payload is related metadata, not the primary artifact body.
                """,
                contentHash: "content-hash",
                injectedContent: "Injected skill content",
                injectedContentHash: "injected-hash",
                sourcePath: "/preview/SKILL.md",
                sourceDescription: "Preview skill",
                bundleManifest: nil,
                role: "architecture-only",
                specializationSummary: nil,
                injectionPolicy: .prependToSystemPrompt
            )
            run.resolvedSkillsJSON = try? JSONEncoder().encode(["proposal_review_triad": resolvedSkill])

            let artifact = Artifact(
                name: "proposal_review_architect",
                contractID: "proposal_review_architect_v1",
                format: .markdown,
                filePath: artifactURL.path,
                runID: run.id,
                stageID: stage.stageID,
                agentID: agentExecution.agentID,
                provider: agentExecution.provider
            )
            artifact.agentExecution = agentExecution

            context.insert(run)
            context.insert(agentExecution)
            context.insert(artifact)
        }
    }

    let container = seededContainer()
    let context = container.mainContext
    let run = try! context.fetch(FetchDescriptor<Run>()).first!
    let artifact = try! context.fetch(FetchDescriptor<Artifact>()).first!

    return ArtifactInspectorView(artifact: artifact, run: run)
        .environment(\.modelContext, context)
        .modelContainer(container)
        .frame(width: 920, height: 560)
}
