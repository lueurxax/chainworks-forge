import SwiftUI
import SwiftData

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
    @State private var isPinned: Bool = false

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

                Divider()

                // §9.2: Provenance chips
                provenanceChips

                Divider()

                // §9.3: Produced-by / consumed-by
                traceabilitySection

                Divider()

                // §9.1: Format-aware rendering
                if let content {
                    formatAwareRenderer(content: content, format: artifact.format)
                } else {
                    ContentUnavailableView(
                        "Content Unavailable",
                        systemImage: "doc.questionmark",
                        description: Text("Could not load artifact from disk.")
                    )
                }

                Divider()

                // §9.5: Open actions
                openActions
            }
            .padding()
        }
        .frame(minWidth: 640, minHeight: 480)
        .accessibilityIdentifier("artifact-inspector-view")
        .navigationTitle("Artifact Inspector")
        .task {
            content = try? String(contentsOfFile: artifact.filePath, encoding: .utf8)
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
            if let model = artifact.model {
                provenanceChip("Model", value: model, icon: "cpu")
            }
            if let effort = artifact.effort {
                provenanceChip("Effort", value: effort, icon: "gauge.medium")
            }
            provenanceChip("Attempt", value: "#\(artifact.attemptNumber)", icon: "arrow.clockwise")
            provenanceChip("Trust", value: run.runtimeTrustLevel ?? "unknown", icon: "shield")
        }
    }

    private func provenanceChip(_ label: String, value: some StringProtocol, icon: String) -> some View {
        HStack(spacing: 3) {
            Image(systemName: icon)
                .font(.caption2)
            Text("\(label): \(value)")
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

            // Consumed-by (via input bindings)
            if let agentExec = artifact.agentExecution,
               let inputData = agentExec.consumedInputArtifactNamesJSON,
               let inputNames = try? JSONDecoder().decode([String].self, from: inputData) {
                ForEach(inputNames, id: \.self) { inputName in
                    HStack {
                        Image(systemName: "arrow.left.circle")
                            .foregroundStyle(.blue)
                        Text("Consumed input:")
                            .font(.caption)
                        Text(inputName)
                            .font(.caption.monospaced())
                    }
                }
            }
        }
    }

    // MARK: - Format-Aware Rendering (§9.1)

    @ViewBuilder
    private func formatAwareRenderer(content: String, format: ArtifactFormat) -> some View {
        switch format {
        case .markdown:
            Text(content)
                .font(.body)
                .textSelection(.enabled)

        case .json:
            // Pretty-print JSON
            Text(prettyPrintJSON(content))
                .font(.body.monospaced())
                .textSelection(.enabled)

        case .diff:
            VStack(alignment: .leading, spacing: 1) {
                ForEach(Array(content.components(separatedBy: .newlines).enumerated()), id: \.offset) { _, line in
                    Text(line)
                        .font(.body.monospaced())
                        .foregroundStyle(diffLineColor(line))
                        .background(diffLineBackground(line))
                }
            }
            .textSelection(.enabled)

        case .report:
            Text(content)
                .font(.body.monospaced())
                .textSelection(.enabled)
        }
    }

    private func prettyPrintJSON(_ raw: String) -> String {
        guard let data = raw.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data),
              let pretty = try? JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys]) else {
            return raw
        }
        return String(data: pretty, encoding: .utf8) ?? raw
    }

    private func diffLineColor(_ line: String) -> Color {
        if line.hasPrefix("+") { return .green }
        if line.hasPrefix("-") { return .red }
        if line.hasPrefix("@@") { return .blue }
        return .primary
    }

    private func diffLineBackground(_ line: String) -> Color {
        if line.hasPrefix("+") { return .green.opacity(0.1) }
        if line.hasPrefix("-") { return .red.opacity(0.1) }
        return .clear
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
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(artifact.filePath, forType: .string)
            }
        }
        .buttonStyle(.bordered)
    }

    // MARK: - Pin / Unpin (§9.4)

    private func togglePin() {
        isPinned.toggle()
        artifact.isPinned = isPinned
        try? modelContext.save()
    }

    private var formatColor: Color {
        switch artifact.format {
        case .markdown: return .blue
        case .json: return .orange
        case .diff: return .green
        case .report: return .purple
        }
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
