// P073 §J: Diagnostics detail view — 3×4 stability budget grid.
//
// Layout adapts to window width:
//   ≥ 720 pt  → 3 columns  (canonical)
//   ≥ 440 pt  → 2 columns
//   < 440 pt  → 1 column   (edge case / Split View)
//
// Advisory-missing treatment (§J3): cells with measurementStatus="missing"
// use a greyed background, display "Awaiting telemetry" in place of a value,
// and render the status pill in neutral grey.

import SwiftUI

struct DiagnosticsDetailView: View {
    @StateObject private var vm = StabilityBudgetViewModel.bootstrap()
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: ForgeSpacing.section) {
                header
                if let budget = vm.budget, !budget.metrics.isEmpty {
                    budgetGrid(budget.metrics)
                } else if vm.lastError != nil {
                    errorPanel
                } else {
                    loadingPanel
                }
            }
            .padding(ForgeSpacing.large)
        }
        .navigationTitle("Stability Diagnostics")
        .task { await vm.refresh() }
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Done") { dismiss() }
            }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.small) {
            Label("Stability Budget", systemImage: "chart.bar.doc.horizontal")
                .font(.title3.weight(.semibold))
            Text("P073 regression budget — 12 metrics, single authoritative materializer.")
                .font(.callout)
                .foregroundStyle(ForgeColor.Text.secondary)
        }
    }

    @ViewBuilder
    private func budgetGrid(_ rows: [StabilityBudgetRowPayload]) -> some View {
        GeometryReader { geo in
            let columns = columnCount(width: geo.size.width)
            LazyVGrid(
                columns: Array(repeating: GridItem(.flexible(), spacing: ForgeSpacing.small), count: columns),
                spacing: ForgeSpacing.small
            ) {
                ForEach(rows) { row in
                    StabilityMetricCell(row: row)
                }
            }
        }
        .frame(minHeight: 400)
    }

    private func columnCount(width: CGFloat) -> Int {
        if width >= 720 { return 3 }
        if width >= 440 { return 2 }
        return 1
    }

    private var loadingPanel: some View {
        HStack(spacing: ForgeSpacing.small) {
            ProgressView()
                .controlSize(.small)
            Text("Loading stability metrics…")
                .font(.callout)
                .foregroundStyle(ForgeColor.Text.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(ForgeSpacing.large)
        .forgePanel()
    }

    private var errorPanel: some View {
        HStack(spacing: ForgeSpacing.small) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(ForgeStatusColor.warning)
            VStack(alignment: .leading, spacing: 2) {
                Text("Could not load stability budget")
                    .font(.callout.weight(.medium))
                if let err = vm.lastError {
                    Text(err.description)
                        .font(.caption)
                        .foregroundStyle(ForgeColor.Text.secondary)
                        .textSelection(.enabled)
                }
            }
            Spacer(minLength: 0)
            Button("Retry") { Task { await vm.refresh() } }
                .controlSize(.small)
        }
        .padding(ForgeSpacing.large)
        .forgePanel(tint: ForgeStatusColor.warning)
    }
}

// MARK: - Metric cell

private struct StabilityMetricCell: View {
    let row: StabilityBudgetRowPayload

    private var isMissing: Bool { row.measurementStatus == "missing" }
    private var isBlocking: Bool { row.blockingMode == "blocking" }

    private var statusColor: Color {
        if isMissing { return ForgeStatusColor.neutral }
        switch row.measurementStatus {
        case "present": return ForgeStatusColor.success
        case "stale": return ForgeStatusColor.warning
        default: return ForgeStatusColor.neutral
        }
    }

    private var modeLabel: String {
        switch row.blockingMode {
        case "blocking": return "blocking"
        case "blocking_after_condition": return "cond."
        case "advisory": return "advisory"
        case "advisory_until_p038": return "adv/P038"
        default: return row.blockingMode
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: ForgeSpacing.compact) {
            HStack(spacing: ForgeSpacing.compact) {
                Text(row.metricId)
                    .font(.caption.weight(.bold).monospaced())
                    .foregroundStyle(ForgeColor.Text.primary)
                Spacer(minLength: 0)
                statusPill
            }

            if isMissing {
                Text("Awaiting telemetry")
                    .font(.caption)
                    .foregroundStyle(ForgeColor.Text.tertiary)
                    .italic()
            } else if let value = row.currentValue {
                Text(formatValue(value))
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(ForgeColor.Text.primary)
            }

            HStack(spacing: ForgeSpacing.compact) {
                Text(row.targetThreshold)
                    .font(.caption2)
                    .foregroundStyle(ForgeColor.Text.tertiary)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 0)
                Text(modeLabel)
                    .font(.caption2)
                    .foregroundStyle(isBlocking ? ForgeStatusColor.error : ForgeColor.Text.tertiary)
            }
        }
        .padding(ForgeSpacing.small)
        .background(
            isMissing ? ForgeColor.Surface.muted : statusColor.opacity(0.07),
            in: RoundedRectangle(cornerRadius: ForgeRadius.card, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: ForgeRadius.card, style: .continuous)
                .strokeBorder(statusColor.opacity(isMissing ? 0.15 : 0.3), lineWidth: 1)
        )
        .accessibilityIdentifier("stability-metric-\(row.metricId)")
    }

    private var statusPill: some View {
        let label: String = isMissing ? "—" : row.measurementStatus.replacingOccurrences(of: "_", with: " ")
        return Text(label)
            .font(.caption2.weight(.medium))
            .foregroundStyle(statusColor)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(statusColor.opacity(0.15), in: Capsule())
    }

    private func formatValue(_ v: Double) -> String {
        if v == v.rounded() && abs(v) < 1_000_000 {
            return String(format: "%.0f", v)
        }
        return String(format: "%.2f", v)
    }
}
