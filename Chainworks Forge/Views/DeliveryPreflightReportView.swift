import SwiftUI

// MARK: - DeliveryPreflightReportView (Proposal 007 — §9.6)

/// Shows delivery preflight check results before starting a repo-backed run.
/// Extends the provider-platform baseline with repo/release-specific checks.
struct DeliveryPreflightReportView: View {
    let result: DeliveryPreflightService.PreflightResult

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: result.passed ? "checkmark.shield.fill" : "exclamationmark.shield.fill")
                    .foregroundStyle(result.passed ? .green : .orange)
                Text("Delivery Preflight")
                    .font(.headline)
                Spacer()
                Text(result.passed ? "Ready" : "Issues Found")
                    .font(.caption.bold())
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(result.passed ? Color.green.opacity(0.15) : Color.orange.opacity(0.15))
                    .foregroundStyle(result.passed ? .green : .orange)
                    .clipShape(Capsule())
            }

            ForEach(result.checks, id: \.id) { check in
                HStack(spacing: 8) {
                    Image(systemName: check.passed ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundStyle(check.passed ? .green : .red)
                        .font(.body)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(check.label)
                            .font(.body)
                        if let detail = check.detail {
                            Text(detail)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                        }
                    }

                    Spacer()
                }
                .padding(.vertical, 2)
            }

            if !result.passed {
                Divider()

                VStack(alignment: .leading, spacing: 4) {
                    Text("Failed Checks")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach(result.failedChecks, id: \.id) { check in
                        HStack(spacing: 4) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .font(.caption2)
                                .foregroundStyle(.orange)
                            Text(check.label)
                                .font(.caption)
                                .foregroundStyle(.red)
                            if let detail = check.detail {
                                Text("— \(detail)")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                    }
                }
            }
        }
        .padding()
        .accessibilityIdentifier("delivery-preflight-report-view")
    }
}

// MARK: - Preview

#Preview("Delivery Preflight — All Passing") {
    DeliveryPreflightReportView(
        result: DeliveryPreflightService.PreflightResult(
            checks: [
                .init(id: "repo_root", label: "Repository root exists", passed: true, detail: "/Users/user/Documents/Chainworks Forge"),
                .init(id: "git_repo", label: "Valid git repository", passed: true, detail: nil),
                .init(id: "base_branch", label: "Base branch 'main' exists", passed: true, detail: nil),
                .init(id: "worktree_writable", label: "Worktree base path is writable", passed: true, detail: "/Users/user/Library/Application Support/Chainworks Forge/worktrees"),
                .init(id: "release_target", label: "Release target configured", passed: true, detail: "Local Sandbox (sandbox)"),
                .init(id: "repo_identifier", label: "Repository identifier set", passed: true, detail: "user/chainworks-forge"),
            ],
            passed: true,
            timestamp: Date()
        )
    )
    .frame(width: 520)
}

#Preview("Delivery Preflight — Issues Found") {
    DeliveryPreflightReportView(
        result: DeliveryPreflightService.PreflightResult(
            checks: [
                .init(id: "repo_root", label: "Repository root exists", passed: true, detail: "/Users/user/Documents/Chainworks Forge"),
                .init(id: "git_repo", label: "Valid git repository", passed: true, detail: nil),
                .init(id: "base_branch", label: "Base branch 'release/v2' exists", passed: false, detail: "Branch 'release/v2' not found"),
                .init(id: "worktree_writable", label: "Worktree base path is writable", passed: true, detail: nil),
                .init(id: "release_target", label: "Release target configured", passed: false, detail: "No release target specified"),
                .init(id: "repo_identifier", label: "Repository identifier set", passed: true, detail: "user/chainworks-forge"),
            ],
            passed: false,
            timestamp: Date()
        )
    )
    .frame(width: 520)
}
