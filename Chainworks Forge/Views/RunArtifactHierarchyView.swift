import SwiftUI
import SwiftData

struct RunArtifactHierarchyView: View {
    let hierarchy: RunArtifactHierarchy
    let onOpenArtifact: (Artifact) -> Void
    let artifactResolver: (UUID) -> Artifact?
    var promotedTitle: String = "Promoted Artifacts"

    @State private var searchText = ""
    @State private var selectedStageID = "all"
    @State private var selectedAgentID = "all"
    @State private var selectedBucketID = "all"

    private var stageOptions: [(id: String, title: String)] {
        let unique = Dictionary(
            hierarchy.stageGroups.map { ($0.stageID, $0.stageLabel) },
            uniquingKeysWith: { current, _ in current }
        )
        return unique
            .map { ($0.key, $0.value) }
            .sorted { $0.title.localizedStandardCompare($1.title) == .orderedAscending }
    }

    private var filteredStageGroups: [RunArtifactStageGroup] {
        hierarchy.stageGroups.compactMap(filteredStageGroup)
    }

    private var agentOptions: [(id: String, title: String)] {
        let unique = Dictionary(
            hierarchy.stageGroups
                .flatMap(\.agentGroups)
                .map { ($0.agentID, $0.agentTitle) },
            uniquingKeysWith: { current, _ in current }
        )
        return unique
            .map { ($0.key, $0.value) }
            .sorted { $0.title.localizedStandardCompare($1.title) == .orderedAscending }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                filterControls

                if promotedArtifacts.isEmpty == false {
                    promotedRow
                }

                if filteredStageGroups.isEmpty {
                    ContentUnavailableView(
                        "No Matching Artifacts",
                        systemImage: "shippingbox",
                        description: Text("Adjust the filters to inspect a different stage, artifact type, or file name.")
                    )
                } else {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        ForEach(filteredStageGroups) { stageGroup in
                            stageGroupSection(stageGroup)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding()
        }
        .accessibilityIdentifier("run-artifact-hierarchy-view")
    }

    private var filterControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Picker("Stage", selection: $selectedStageID) {
                    Text("All Stages").tag("all")
                    ForEach(stageOptions, id: \.id) { option in
                        Text(option.title).tag(option.id)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("artifact-hierarchy-stage-filter")

                Picker("Agent", selection: $selectedAgentID) {
                    Text("All Agents").tag("all")
                    ForEach(agentOptions, id: \.id) { option in
                        Text(option.title).tag(option.id)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("artifact-hierarchy-agent-filter")

                Picker("Type", selection: $selectedBucketID) {
                    Text("All Types").tag("all")
                    ForEach(RunArtifactBucketKind.allCases) { bucket in
                        Text(bucket.title).tag(bucket.rawValue)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("artifact-hierarchy-bucket-filter")
            }

            TextField("Filter by artifact name", text: $searchText)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("artifact-hierarchy-search")
        }
    }

    private var promotedArtifacts: [RunArtifactLeaf] {
        hierarchy.promotedArtifacts.filter(matchesFilters)
    }

    private var promotedRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(promotedTitle)
                .font(.headline)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(promotedArtifacts) { leaf in
                        artifactChip(for: leaf)
                    }
                }
                .padding(.vertical, 2)
            }
        }
    }

    private func stageGroupSection(_ stageGroup: RunArtifactStageGroup) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                if stageGroup.stageBuckets.isEmpty == false {
                    bucketGroupList(stageGroup.stageBuckets, titlePrefix: "Stage")
                }

                ForEach(stageGroup.agentGroups) { agentGroup in
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text(agentGroup.agentTitle)
                                .font(.subheadline.weight(.semibold))
                            Spacer()
                            Text(agentGroup.agentID)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }

                        bucketGroupList(agentGroup.semanticBuckets, titlePrefix: nil)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(stageGroup.stageLabel)
                        .font(.subheadline.bold())
                    Text("\(stageGroup.stageID) · iteration \(stageGroup.iteration) · attempt \(stageGroup.attemptNumber)")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text("\(stageGroup.allArtifacts.count) artifacts")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func bucketGroupList(_ buckets: [RunArtifactSemanticBucket], titlePrefix: String?) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(buckets) { bucket in
                DisclosureGroup {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(bucket.artifacts) { leaf in
                            artifactRow(for: leaf)
                        }
                    }
                    .padding(.top, 6)
                } label: {
                    HStack {
                        Text([titlePrefix, bucket.bucket.title].compactMap { $0 }.joined(separator: " "))
                        Spacer()
                        Text("\(bucket.artifacts.count)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .font(.caption.weight(.semibold))
                }
            }
        }
    }

    private func artifactRow(for leaf: RunArtifactLeaf) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Button {
                if let artifact = artifactResolver(leaf.artifactID) {
                    onOpenArtifact(artifact)
                }
            } label: {
                HStack(alignment: .top, spacing: 10) {
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(leaf.name)
                                .font(.subheadline)
                            if leaf.isLatestSummaryReport {
                                statusBadge("Latest Summary", color: DesignTokens.Status.warning)
                            }
                            if leaf.isLatestImmutableReport {
                                statusBadge("Immutable Report", color: DesignTokens.Status.success)
                            }
                            if leaf.isPinned {
                                statusBadge("Pinned", color: DesignTokens.Action.primary)
                            }
                        }

                        Text("\(leaf.contractID) · \(leaf.format.rawValue)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)

                        if let reportKind = leaf.reportKind {
                            Text("reportKind: \(reportKind)\(leaf.reportVersion.map { " · v\($0)" } ?? "")")
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                    }

                    Spacer()

                    Text(leaf.createdAt, format: .dateTime.hour().minute().second())
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("artifact-button-\(leaf.name)")

            Button {
                if let path = artifactResolver(leaf.artifactID)?.filePath ?? leaf.fileURL?.path {
                    ArtifactPathClipboard.copy(path: path)
                }
            } label: {
                Label("Copy Path", systemImage: "doc.on.clipboard")
                    .labelStyle(.iconOnly)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel("Copy path for \(leaf.name)")
            .disabled((artifactResolver(leaf.artifactID)?.filePath ?? leaf.fileURL?.path) == nil)
            .accessibilityIdentifier("artifact-copy-path-\(leaf.name)")
        }
    }

    private func artifactChip(for leaf: RunArtifactLeaf) -> some View {
        Button {
            if let artifact = artifactResolver(leaf.artifactID) {
                onOpenArtifact(artifact)
            }
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                Text(leaf.name)
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)
                Text("\(leaf.stageLabel) · \(leaf.agentTitle)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(width: 220, alignment: .leading)
            .padding(10)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("artifact-promoted-\(leaf.name)")
    }

    private func statusBadge(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.14), in: Capsule())
            .foregroundStyle(color)
    }

    private func filteredStageGroup(_ stageGroup: RunArtifactStageGroup) -> RunArtifactStageGroup? {
        guard selectedStageID == "all" || stageGroup.stageID == selectedStageID else {
            return nil
        }

        let stageBuckets: [RunArtifactSemanticBucket] = stageGroup.stageBuckets.compactMap(filteredBucket)
        let agentGroups: [RunArtifactAgentGroup] = stageGroup.agentGroups.compactMap { agentGroup -> RunArtifactAgentGroup? in
            let buckets = agentGroup.semanticBuckets.compactMap(filteredBucket)
            guard buckets.isEmpty == false else { return nil }
            return RunArtifactAgentGroup(
                agentExecutionID: agentGroup.agentExecutionID,
                agentID: agentGroup.agentID,
                agentTitle: agentGroup.agentTitle,
                semanticBuckets: buckets
            )
        }

        guard stageBuckets.isEmpty == false || agentGroups.isEmpty == false else {
            return nil
        }

        return RunArtifactStageGroup(
            stageExecutionID: stageGroup.stageExecutionID,
            stageID: stageGroup.stageID,
            stageLabel: stageGroup.stageLabel,
            iteration: stageGroup.iteration,
            attemptNumber: stageGroup.attemptNumber,
            stageBuckets: stageBuckets,
            agentGroups: agentGroups
        )
    }

    private func filteredBucket(_ bucket: RunArtifactSemanticBucket) -> RunArtifactSemanticBucket? {
        guard selectedBucketID == "all" || bucket.bucket.rawValue == selectedBucketID else {
            return nil
        }

        let artifacts = bucket.artifacts.filter(matchesFilters)
        guard artifacts.isEmpty == false else { return nil }
        return RunArtifactSemanticBucket(bucket: bucket.bucket, artifacts: artifacts)
    }

    private func matchesFilters(_ leaf: RunArtifactLeaf) -> Bool {
        guard selectedAgentID == "all" || leaf.agentID == selectedAgentID else {
            return false
        }

        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard query.isEmpty == false else { return true }

        let haystack = [
            leaf.name,
            leaf.contractID,
            leaf.stageLabel,
            leaf.agentTitle
        ].joined(separator: " ").lowercased()
        return haystack.contains(query.lowercased())
    }
}
