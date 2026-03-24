import SwiftUI
import SwiftData

// MARK: - P005-OPS §10: Menu Bar Status View

/// Optional menu bar extra showing operator attention state.
/// Shows waiting approvals + blocked runs count with quick actions.
struct MenuBarStatusView: View {
    @Environment(ExecutionService.self) private var executionService

    @Query(sort: \Run.startedAt, order: .reverse)
    private var allRuns: [Run]

    private var attentionRuns: [Run] {
        allRuns.filter { $0.status == .waitingApproval || $0.status == .blocked }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Chainworks Forge")
                .font(.headline)

            Divider()

            if attentionRuns.isEmpty {
                Label("All clear", systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.green)
            } else {
                ForEach(attentionRuns.prefix(5)) { run in
                    HStack {
                        Image(systemName: run.status == .waitingApproval ? "bell.badge.fill" : "exclamationmark.triangle.fill")
                            .foregroundStyle(run.status == .waitingApproval ? .orange : .red)
                        VStack(alignment: .leading) {
                            Text(run.idea?.title ?? "Run")
                                .font(.caption)
                                .lineLimit(1)
                            Text(run.status.rawValue)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }

                if attentionRuns.count > 5 {
                    Text("+ \(attentionRuns.count - 5) more")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Divider()

            HStack {
                Image(systemName: "circle.fill")
                    .font(.caption2)
                    .foregroundStyle(executionService.hasActiveRuns ? .green : .secondary)
                Text(executionService.hasActiveRuns ? "Engine active" : "Idle")
                    .font(.caption)
            }
        }
        .padding()
        .frame(width: 250)
    }
}
