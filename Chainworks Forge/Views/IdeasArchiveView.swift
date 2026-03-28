import SwiftUI

struct IdeaLifecycleBadge: View {
    let idea: Idea

    var body: some View {
        Text(idea.archiveLifecycleStatus)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(backgroundColor.opacity(0.18), in: Capsule())
            .foregroundStyle(backgroundColor)
    }

    private var backgroundColor: Color {
        if idea.isArchived { return .secondary }
        if let latestRun = idea.latestRun {
            switch latestRun.presentationStatus {
            case .pending:
                return idea.status == .draft ? .blue : .gray
            case .ready:
                return .gray
            case .running:
                return .green
            case .waitingApproval, .cancelling:
                return .orange
            case .blocked:
                return .yellow
            case .completed:
                return .mint
            case .failed:
                return .red
            case .cancelled:
                return .gray
            }
        }
        switch idea.status {
        case .draft:
            return .blue
        case .active:
            return .green
        case .completed:
            return .mint
        case .failed:
            return .red
        }
    }
}

struct IdeasArchiveView: View {
    let ideas: [Idea]
    let searchText: String
    let onRestore: (Idea) -> Void

    private var filteredIdeas: [Idea] {
        guard !searchText.isEmpty else { return ideas }
        return ideas.filter {
            $0.title.localizedCaseInsensitiveContains(searchText)
            || $0.body.localizedCaseInsensitiveContains(searchText)
        }
    }

    var body: some View {
        Group {
            if filteredIdeas.isEmpty {
                ContentUnavailableView(
                    "No archived ideas",
                    systemImage: "archivebox",
                    description: Text(searchText.isEmpty ? "Archive completed or irrelevant ideas to keep the active lane focused." : "No archived ideas match the current search.")
                )
            } else {
                List(filteredIdeas) { idea in
                    HStack(alignment: .top, spacing: 12) {
                        NavigationLink {
                            IdeaDetailView(idea: idea)
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(idea.title)
                                    .font(.headline)
                                Text(idea.body)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                                HStack(spacing: 8) {
                                    IdeaLifecycleBadge(idea: idea)
                                    if let archivedAt = idea.archivedAt {
                                        Text("Archived \(archivedAt.formatted(date: .abbreviated, time: .omitted))")
                                            .font(.caption2)
                                            .foregroundStyle(.tertiary)
                                    }
                                }
                            }
                        }
                        .buttonStyle(.plain)
                        Spacer(minLength: 12)
                        Button("Restore") {
                            onRestore(idea)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityIdentifier("idea-restore-\(idea.title)")
                    }
                    .padding(.vertical, 4)
                    .contextMenu {
                        Button("Restore") {
                            onRestore(idea)
                        }
                    }
                }
                .accessibilityIdentifier("ideas-archive-list")
            }
        }
    }
}
