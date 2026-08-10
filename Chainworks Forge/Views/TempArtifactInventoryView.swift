import SwiftUI
import AppKit

/// P089: Temporary artifact inventory surface for Run Report > Diagnostics > Temporary Artifacts.
/// Gated by TempArtifactDiagnosticsVisibilityStore backed by UserDefaults domain com.chainworks.forge.
/// Hidden surface performs no scan. All filesystem data comes from the backend; Swift never scans paths.
struct TempArtifactInventoryView: View {
    @State private var viewModel: TempArtifactInventoryViewModel
    private let runID: String
    private let visibilityStore: TempArtifactDiagnosticsVisibilityStore
    private let pasteboardWriter: any TempArtifactRowPasteboardWriting

    @State private var selectedRowID: String? = nil
    @State private var contextMenuTargetRowID: String? = nil
    @FocusState private var diagnosticsFocus: DiagnosticsFocus?

    private enum DiagnosticsFocus: Hashable {
        case refresh
        case cancel
        case copy
        case table
    }

    init(
        runID: String,
        visibilityStore: TempArtifactDiagnosticsVisibilityStore = TempArtifactDiagnosticsVisibilityStore(),
        viewModel: TempArtifactInventoryViewModel? = nil,
        pasteboardWriter: (any TempArtifactRowPasteboardWriting)? = nil
    ) {
        self.runID = runID
        self.visibilityStore = visibilityStore
        self._viewModel = State(initialValue: viewModel ?? TempArtifactInventoryViewModel())
        self.pasteboardWriter = pasteboardWriter ?? TempArtifactRowPasteboardWriter()
    }

    /// The local preference alone is not authoritative: it cannot reflect the
    /// daemon's actual `CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE`. Once a real
    /// backend response has been accepted, the surface stays hidden unless that
    /// response's `mode` is `operator_visible` — closing the visibility-mode
    /// bypass where a stale local `true` preference could expose the UI while the
    /// backend is actually in `hidden_readback` (or `disabled`).
    private var isSurfaceVisible: Bool {
        visibilityStore.isVisible && viewModel.isBackendAuthorizedForVisibleSurface
    }

    var body: some View {
        Group {
            if isSurfaceVisible {
                visibleContent
            } else {
                EmptyView()
                    .accessibilityIdentifier("temp-artifact-inventory-hidden")
            }
        }
        .accessibilityIdentifier("temp-artifact-inventory-root")
        .focusedSceneValue(
            \.tempArtifactInventoryCopyCommandState,
            TempArtifactInventoryCopyCommandState(canCopy: canCopySelectedRow)
        )
        .focusedSceneValue(
            \.tempArtifactInventoryCopyCommandActions,
            TempArtifactInventoryCopyCommandActions(copyRedactedRow: copySelectedRow)
        )
        .task(id: visibilityStore.isVisible) {
            guard visibilityStore.isVisible else { return }
            viewModel.resolveBackendVisibility(runID: runID)
        }
        .onChange(of: viewModel.selectedRowIdentity?.value) { _, identity in
            selectedRowID = identity
        }
    }

    // MARK: - Visible surface

    private var visibleContent: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
            headerBar
            stateContent
        }
        .onAppear {
            diagnosticsFocus = .refresh
            viewModel.setSceneActivity(isVisible: true, isFocused: true)
        }
        .onChange(of: diagnosticsFocus) { _, focus in
            viewModel.setSceneActivity(isVisible: true, isFocused: focus != nil)
        }
        .onDisappear { viewModel.onSceneClose() }
        .accessibilityIdentifier("temp-artifact-inventory-content")
    }

    private var headerBar: some View {
        HStack(spacing: ForgeSpacing.small) {
            Text("Temporary Artifacts")
                .font(.headline)
                .accessibilityIdentifier("temp-artifact-inventory-title")
            Spacer()
            if viewModel.inFlightGenerationID != nil {
                Button("Cancel Refresh") {
                    viewModel.cancelRefresh()
                }
                .focused($diagnosticsFocus, equals: .cancel)
                .accessibilityIdentifier("temp-artifact-cancel-refresh")
            }
            Button("Copy Redacted Row") {
                copySelectedRow()
            }
            .disabled(!canCopySelectedRow)
            .focused($diagnosticsFocus, equals: .copy)
            .accessibilityIdentifier("temp-artifact-copy-redacted-row")
            Button("Refresh Preview") {
                viewModel.beginRefresh(runID: runID)
            }
            .disabled(viewModel.inFlightGenerationID != nil)
            .focused($diagnosticsFocus, equals: .refresh)
            .accessibilityIdentifier("temp-artifact-refresh-preview")
        }
    }

    @ViewBuilder
    private var stateContent: some View {
        switch viewModel.viewState {
        case .firstLoad:
            firstLoadPlaceholder
        case .loadingWithoutPrior:
            loadingIndicator
        case .loadingOverStale:
            staleLoadingRow
        case .completeWithRows:
            inventoryResults
        case .completeEmpty:
            VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
                summaryCounterLayout
                emptyResultPlaceholder
            }
        case .partialTimeoutCancelled:
            VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                bannersSection(isError: false)
                if !viewModel.displayRows.isEmpty { inventoryResults }
            }
        case .error:
            VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                bannersSection(isError: true)
                if !viewModel.displayRows.isEmpty { inventoryResults }
            }
        case .disabled(let reasonCode):
            disabledPlaceholder(reasonCode: reasonCode)
        case .busy:
            VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                ForgeWarningBanner(
                    "Scan capacity is currently busy. Please try again shortly.",
                    systemImage: "clock.badge.xmark",
                    tint: .orange
                )
                .accessibilityIdentifier("temp-artifact-busy")
                if !viewModel.displayRows.isEmpty { inventoryResults }
            }
        }
    }

    // MARK: - Placeholders

    private var firstLoadPlaceholder: some View {
        VStack(alignment: .center, spacing: ForgeSpacing.medium) {
            Image(systemName: "folder.badge.questionmark")
                .font(.largeTitle)
                .foregroundStyle(ForgeColor.Text.secondary)
            Text("No preview yet")
                .font(.headline)
            Label("Inventory capability available", systemImage: "checkmark.shield")
                .font(.callout)
                .foregroundStyle(ForgeColor.Text.secondary)
                .accessibilityIdentifier("temp-artifact-capability-status")
            Text("Tap Refresh Preview to scan temporary artifact roots.")
                .font(.callout)
                .foregroundStyle(ForgeColor.Text.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(ForgeSpacing.large)
        .onAppear { diagnosticsFocus = .refresh }
        .accessibilityIdentifier("temp-artifact-first-load")
    }

    private var loadingIndicator: some View {
        VStack(alignment: .center, spacing: ForgeSpacing.medium) {
            ProgressView()
            Text("Scanning…")
                .font(.callout)
                .foregroundStyle(ForgeColor.Text.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(ForgeSpacing.large)
        .accessibilityIdentifier("temp-artifact-loading")
    }

    private var staleLoadingRow: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.small) {
            HStack(spacing: ForgeSpacing.small) {
                ProgressView()
                Image(systemName: "clock.badge.exclamationmark")
                    .foregroundStyle(.orange)
                    .accessibilityLabel("Stale")
                Text("Refreshing — prior results shown")
                    .font(.callout)
                    .foregroundStyle(ForgeColor.Text.secondary)
            }
            .accessibilityIdentifier("temp-artifact-stale-badge")
            inventoryResults
        }
    }

    private var emptyResultPlaceholder: some View {
        VStack(alignment: .center, spacing: ForgeSpacing.medium) {
            Image(systemName: "tray")
                .font(.largeTitle)
                .foregroundStyle(ForgeColor.Text.secondary)
            Text("No temporary artifacts found")
                .font(.headline)
        }
        .frame(maxWidth: .infinity)
        .padding(ForgeSpacing.large)
        .accessibilityIdentifier("temp-artifact-empty-result")
    }

    private func disabledPlaceholder(reasonCode: String?) -> some View {
        VStack(alignment: .center, spacing: ForgeSpacing.medium) {
            Image(systemName: "nosign")
                .font(.largeTitle)
                .foregroundStyle(ForgeColor.Text.secondary)
            Text("Temporary artifact inventory is disabled")
                .font(.headline)
            if let code = reasonCode {
                Text("Reason: \(code)")
                    .font(.callout)
                    .foregroundStyle(ForgeColor.Text.secondary)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(ForgeSpacing.large)
        .accessibilityIdentifier("temp-artifact-disabled")
    }

    // MARK: - Banner stack (vertically scrollable after 3 compact banners or 144 px,
    // whichever is smaller; every error remains reachable by scrolling, none are
    // dropped — see proposal ux_ui_notes.banner_stack).

    private func bannersSection(isError: Bool) -> some View {
        let errors = viewModel.topLevelErrors
        let visibleBannerCount = min(errors.count, 3)
        let visibleHeight = min(CGFloat(visibleBannerCount) * 48, 144)
        return ScrollView {
            VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                ForEach(Array(errors.enumerated()), id: \.offset) { _, entry in
                    let msg = "\(entry.code): \(entry.message)"
                    if isError {
                        ForgeWarningBanner.error(msg)
                    } else {
                        ForgeWarningBanner(msg, tint: .orange)
                    }
                }
            }
        }
        .frame(maxHeight: visibleHeight)
        .accessibilityIdentifier("temp-artifact-banner-stack")
    }

    // MARK: - Results, summary, table, and inspector

    private var inventoryResults: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
            summaryCounterLayout
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: ForgeSpacing.medium) {
                    rowTable
                        .frame(minWidth: 560)
                    if let row = viewModel.selectedRow {
                        selectedRowInspector(row)
                            .frame(width: 320)
                    }
                }
                .frame(minWidth: 900)

                VStack(alignment: .leading, spacing: ForgeSpacing.medium) {
                    rowTable
                    if let row = viewModel.selectedRow {
                        selectedRowInspector(row)
                    }
                }
            }
        }
    }

    private var summaryCounterLayout: some View {
        let groups = metricGroups
        return ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 0) {
                ForEach(Array(groups.enumerated()), id: \.element.id) { index, group in
                    metricGroup(group)
                    if index < groups.count - 1 {
                        Divider()
                            .frame(height: 72)
                    }
                }
            }
            .frame(minWidth: 900, alignment: .leading)

            LazyVGrid(
                columns: [
                    GridItem(.flexible(minimum: 260), alignment: .topLeading),
                    GridItem(.flexible(minimum: 260), alignment: .topLeading),
                ],
                alignment: .leading,
                spacing: ForgeSpacing.small
            ) {
                ForEach(groups) { metricGroup($0) }
            }
            .frame(minWidth: 820)

            VStack(alignment: .leading, spacing: ForgeSpacing.small) {
                ForEach(groups) { metricGroup($0) }
            }
        }
        .accessibilityIdentifier("temp-artifact-summary-counters")
    }

    private var metricGroups: [TempArtifactMetricGroup] {
        guard let payload = viewModel.displayPayload else { return [] }
        let summary = payload.summary
        return [
            TempArtifactMetricGroup(
                id: "totals",
                title: "Totals",
                metrics: [
                    .init(label: "Trees", value: "\(summary.artifactTreeCount)"),
                    .init(label: "Bytes", value: summary.estimatedBytes),
                ]
            ),
            TempArtifactMetricGroup(
                id: "classification",
                title: "Classification",
                metrics: [
                    .init(label: "Active / recent", value: "\(summary.activeOrRecentCount)"),
                    .init(label: "Terminal", value: "\(summary.terminalCandidateCount)"),
                    .init(label: "Orphan", value: "\(summary.orphanCandidateCount)"),
                    .init(label: "Legacy", value: "\(summary.legacyUnmanagedCount)"),
                ]
            ),
            TempArtifactMetricGroup(
                id: "dry-run",
                title: "Dry Run",
                metrics: [
                    .init(label: "Candidates", value: "\(summary.dryRunCandidateCount)"),
                    .init(label: "Guard", value: payload.mutationGuard.status),
                ]
            ),
            TempArtifactMetricGroup(
                id: "health",
                title: "Health",
                metrics: [
                    .init(label: "Status", value: payload.status),
                    .init(label: "Generated", value: payload.generatedAt),
                    .init(label: "Errors", value: "\(summary.scanErrorCount)"),
                    .init(label: "Queue ms", value: "\(summary.queueWaitMs)"),
                    .init(label: "Truncated", value: summary.truncated ? "Yes" : "No"),
                ]
            ),
        ]
    }

    private func metricGroup(_ group: TempArtifactMetricGroup) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(group.title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(ForgeColor.Text.secondary)
            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: 112), alignment: .trailing)],
                alignment: .leading,
                spacing: 4
            ) {
                ForEach(group.metrics) { metric in
                    VStack(alignment: .trailing, spacing: 1) {
                        Text(metric.value)
                            .font(.caption.monospacedDigit().weight(.semibold))
                            .lineLimit(1)
                        Text(metric.label)
                            .font(.caption2)
                            .foregroundStyle(ForgeColor.Text.secondary)
                            .lineLimit(1)
                    }
                    .frame(minWidth: 112, minHeight: 28, alignment: .trailing)
                }
            }
        }
        .padding(.horizontal, ForgeSpacing.small)
    }

    private var rowTable: some View {
        let rows = viewModel.displayRows
        let stale = viewModel.isDisplayingStaleRows
        return Table(rows, selection: Binding(
            get: { selectedRowID },
            set: { newID in
                selectedRowID = newID
                if let id = newID, let row = rows.first(where: { $0.id == id }) {
                    viewModel.selectRow(row)
                } else {
                    viewModel.selectRow(nil)
                }
            }
        )) {
            TableColumn("Status") { row in
                if stale {
                    Label("Stale", systemImage: "clock.badge.exclamationmark")
                        .foregroundStyle(.orange)
                } else {
                    Label(row.statusToken, systemImage: "checkmark.circle")
                        .foregroundStyle(ForgeColor.Text.secondary)
                }
            }
            .width(min: 72, ideal: 90)
            TableColumn("Path") { row in
                Text(row.pathDisplay)
                    .font(.caption.monospaced())
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            TableColumn("Classification") { row in
                Text(row.lifecycleClassification)
                    .font(.caption)
            }
            TableColumn("Dry Run") { row in
                Text(row.dryRunRecommendation ?? "—")
                    .font(.caption)
                    .foregroundStyle(ForgeColor.Text.secondary)
            }
            TableColumn("Size") { row in
                Text(row.estimatedSizeBytes + " B")
                    .font(.caption.monospaced())
                    .frame(maxWidth: .infinity, alignment: .trailing)
            }
            .width(80)
            TableColumn("Last Touched") { row in
                Text(row.lastTouchedAt ?? "—")
                    .font(.caption.monospaced())
                    .foregroundStyle(ForgeColor.Text.secondary)
            }
        }
        .contextMenu(forSelectionType: String.self) { ids in
            let targetID = TempArtifactContextMenuTargeting.targetID(
                contextSelection: ids,
                keyboardSelection: selectedRowID
            )
            if let targetID, let row = rows.first(where: { $0.id == targetID }) {
                Button("Copy Redacted Row") {
                    contextMenuTargetRowID = targetID
                    copyRow(row, stale: stale)
                    contextMenuTargetRowID = nil
                }
            }
        } primaryAction: { _ in }
        .frame(minHeight: 200)
        .focused($diagnosticsFocus, equals: .table)
        .accessibilityIdentifier("temp-artifact-table")
    }

    private func selectedRowInspector(
        _ row: TempArtifactInventoryResponse.Row
    ) -> some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.small) {
            Text("Selected Artifact")
                .font(.headline)
            inspectorField("Path", row.pathDisplay)
            inspectorField("Path Hash", row.pathHashShort)
            inspectorField("Correlation", row.correlationKey)
            inspectorField("Root Kind", row.rootKind)
            inspectorField("Artifact Kind", row.artifactKind ?? "—")
            inspectorField("Manifest", row.manifestState ?? "—")
            inspectorField("Classification", row.lifecycleClassification)
            inspectorField("Dry Run", row.dryRunRecommendation ?? "—")
            inspectorField("Estimated Bytes", row.estimatedSizeBytes)
            inspectorField("Last Touched", row.lastTouchedAt ?? "—")
            inspectorField("Process Evidence", row.activeProcessEvidence ?? "—")
            inspectorField("Owner", row.owner ?? "—")
            inspectorField("Owner Inference", row.ownerInference ?? "—")
            inspectorField("Status", row.statusToken)
            inspectorField("Generated", row.generatedAt)
            inspectorField("Stale", viewModel.isDisplayingStaleRows ? "Yes" : "No")
            if !row.partialErrors.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Partial Errors")
                        .font(.caption)
                        .foregroundStyle(ForgeColor.Text.secondary)
                    ForEach(Array(row.partialErrors.prefix(3).enumerated()), id: \.offset) {
                        _, error in
                        Text(error)
                            .font(.caption.monospaced())
                    }
                    if row.partialErrors.count > 3 {
                        DisclosureGroup("Show More") {
                            ForEach(
                                Array(row.partialErrors.dropFirst(3).enumerated()),
                                id: \.offset
                            ) { _, error in
                                Text(error)
                                    .font(.caption.monospaced())
                            }
                        }
                    }
                }
            }
        }
        .padding(ForgeSpacing.medium)
        .background(ForgeColor.Surface.elevated, in: RoundedRectangle(cornerRadius: 8))
        .accessibilityIdentifier("temp-artifact-selected-row-inspector")
    }

    private func inspectorField(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label)
                .font(.caption2)
                .foregroundStyle(ForgeColor.Text.secondary)
            Text(value)
                .font(.caption.monospaced())
                .textSelection(.enabled)
        }
    }

    private var canCopySelectedRow: Bool {
        isSurfaceVisible && viewModel.focusedCopyCommandEnabled && viewModel.selectedRow != nil
    }

    private func copySelectedRow() {
        guard canCopySelectedRow, let row = viewModel.selectedRow else { return }
        copyRow(row, stale: viewModel.isDisplayingStaleRows)
    }

    private func copyRow(_ row: TempArtifactInventoryResponse.Row, stale: Bool) {
        pasteboardWriter.writeRedactedRow(row, stale: stale)
    }
}

private struct TempArtifactMetricGroup: Identifiable {
    struct Metric: Identifiable {
        let label: String
        let value: String
        var id: String { label }
    }

    let id: String
    let title: String
    let metrics: [Metric]
}

enum TempArtifactContextMenuTargeting {
    /// Native Table context-menu selection is independent from keyboard
    /// selection. Prefer the right-clicked row when supplied and never mutate
    /// the keyboard selection merely because the menu opened.
    static func targetID(
        contextSelection: Set<String>,
        keyboardSelection: String?
    ) -> String? {
        contextSelection.first ?? keyboardSelection
    }
}

struct TempArtifactInventoryCopyCommandState {
    var canCopy: Bool = false
}

struct TempArtifactInventoryCopyCommandActions {
    var copyRedactedRow: () -> Void = {}
}

private struct TempArtifactInventoryCopyCommandStateKey: FocusedValueKey {
    typealias Value = TempArtifactInventoryCopyCommandState
}

private struct TempArtifactInventoryCopyCommandActionsKey: FocusedValueKey {
    typealias Value = TempArtifactInventoryCopyCommandActions
}

extension FocusedValues {
    var tempArtifactInventoryCopyCommandState: TempArtifactInventoryCopyCommandState? {
        get { self[TempArtifactInventoryCopyCommandStateKey.self] }
        set { self[TempArtifactInventoryCopyCommandStateKey.self] = newValue }
    }

    var tempArtifactInventoryCopyCommandActions: TempArtifactInventoryCopyCommandActions? {
        get { self[TempArtifactInventoryCopyCommandActionsKey.self] }
        set { self[TempArtifactInventoryCopyCommandActionsKey.self] = newValue }
    }
}

struct TempArtifactInventoryCommands: Commands {
    @FocusedValue(\.tempArtifactInventoryCopyCommandState) private var state
    @FocusedValue(\.tempArtifactInventoryCopyCommandActions) private var actions

    var body: some Commands {
        CommandGroup(before: .pasteboard) {
            Button("Copy Redacted Row") {
                actions?.copyRedactedRow()
            }
            .keyboardShortcut("c", modifiers: .command)
            .disabled(!(state?.canCopy ?? false))
        }
    }
}
