import SwiftUI

// MARK: - DeliveryPreflightReportView (Proposal 007 — §9.6)

/// Shows delivery preflight check results before starting a repo-backed run.
/// Extends the provider-platform baseline with repo/release-specific checks.
struct DeliveryPreflightReportView: View {
    @Environment(\.uiTestAccessibilitySettings) private var uiTestAccessibilitySettings

    let result: DeliveryPreflightService.PreflightResult

    private var statusText: String {
        result.passed ? "Ready" : "Issues Found"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: DesignTokens.Spacing.medium) {
            HStack {
                Image(systemName: result.passed ? "checkmark.shield.fill" : "exclamationmark.shield.fill")
                    .foregroundStyle(result.passed ? DesignTokens.Status.success : DesignTokens.Status.warning)
                ForgeSectionHeader(
                    title: "Delivery Preflight",
                    subtitle: "Repo-backed start checks stay operational first: release readiness, writable worktrees, and target configuration must remain explicit.",
                    systemImage: nil,
                    tint: result.passed ? DesignTokens.Status.success : DesignTokens.Status.warning
                )
                Spacer()
                StatusCapsule(
                    text: statusText,
                    color: result.passed ? DesignTokens.Status.success : DesignTokens.Status.warning,
                    icon: result.passed ? "checkmark.circle.fill" : "exclamationmark.triangle.fill",
                    accessibilityIdentifier: "delivery-preflight-status"
                )
                statusAccessibilityProof
            }

            ForEach(result.checks, id: \.id) { check in
                HStack(spacing: 8) {
                    Image(systemName: check.passed ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundStyle(check.passed ? DesignTokens.Status.success : DesignTokens.Status.error)
                        .font(.body)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(check.label)
                            .font(.body)
                        if let detail = check.detail {
                            Text(detail)
                                .font(DesignTokens.Typography.supporting)
                                .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                .lineLimit(2)
                        }
                    }

                    Spacer()
                }
                .padding(.vertical, 2)
                .forgeInsetPanel(tone: check.passed ? .quiet : .critical)
            }

            if !result.passed {
                Divider()

                VStack(alignment: .leading, spacing: 4) {
                    Text("Failed Checks")
                        .font(DesignTokens.Typography.supporting)
                        .foregroundStyle(DesignTokens.Neutral.textSecondary)
                    ForEach(result.failedChecks, id: \.id) { check in
                        HStack(spacing: 4) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .font(.caption2)
                                .foregroundStyle(DesignTokens.Status.warning)
                            Text(check.label)
                                .font(DesignTokens.Typography.supporting)
                                .foregroundStyle(DesignTokens.Status.error)
                            if let detail = check.detail {
                                Text("— \(detail)")
                                    .font(DesignTokens.Typography.supporting)
                                    .foregroundStyle(DesignTokens.Neutral.textSecondary)
                                    .lineLimit(1)
                            }
                        }
                    }
                }
                .forgeInsetPanel(tone: .warning)
            }
        }
        .forgePanel(tone: result.passed ? .standard : .warning)
        .accessibilityIdentifier("delivery-preflight-report-view")
    }

    @ViewBuilder
    private var statusAccessibilityProof: some View {
        Text(statusText)
            .font(.caption2)
            .frame(width: 1, height: 1)
            .clipped()
            .opacity(0.01)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(statusText)
            .accessibilityValue(accessibilitySettingsDescription)
            .accessibilityIdentifier(statusText)
    }

    private var accessibilitySettingsDescription: String {
        var activeModes: [String] = []
        if uiTestAccessibilitySettings.differentiateWithoutColor {
            activeModes.append("differentiate without color")
        }
        if uiTestAccessibilitySettings.increaseContrast {
            activeModes.append("increase contrast")
        }
        if uiTestAccessibilitySettings.reduceTransparency {
            activeModes.append("reduce transparency")
        }
        return activeModes.isEmpty ? "standard accessibility display settings" : activeModes.joined(separator: ", ")
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
