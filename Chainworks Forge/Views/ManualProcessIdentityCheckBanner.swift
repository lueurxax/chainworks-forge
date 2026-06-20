import SwiftUI
#if canImport(AppKit)
import AppKit
#endif

/// P083 SCORE-LIFT-UI-P083-R66-001: Manual process identity check recovery banner.
///
/// Shown when a provider cancellation intent has intent_state=held and the provider
/// session has process_fate=identity_ambiguous. Per manual_process_identity_check_ui_v1:
/// - No automatic retry spinner.
/// - Explicit operator actions: Copy Diagnostic, Retry Identity Check,
///   Mark Process Absent (copies MCP command — UI action boundary prohibits direct mutation),
///   Open Provider Session Evidence.
/// - VoiceOver reads title, provider name, reason, and the focused action.
/// - Disabled lifecycle commands remain visible with adjacent reason text.
struct ManualProcessIdentityCheckBanner: View {
    let providerName: String
    let sessionId: String
    /// The cancellation_epoch for the held intent (required by manual_process_identity_check_ui_v1
    /// diagnostic payload spec: provider_session_id, cancellation_epoch, process_fate, last_seen_pid,
    /// process_start_identity hash, and latest receipt id).
    let cancellationEpoch: Int?
    let lastSeenPid: Int?
    let processStartIdentityHash: String?
    let latestReceiptId: String?
    let reasonDetail: String?
    let onRetryIdentityCheck: () -> Void
    /// Called when the operator requests Mark Process Absent. Per ui-action-boundary.md, this
    /// mutation routes through the MCP operator command path, not through the SwiftUI layer.
    /// The caller should copy the MCP command or display external operator guidance.
    let onMarkProcessAbsent: () -> Void
    let onOpenProviderSessionEvidence: () -> Void

    @State private var copyConfirmed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            headerRow
            reasonRow
            actionRow
            disabledCommandNote
        }
        .padding(12)
        .background(Color.orange.opacity(0.08), in: RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(Color.orange.opacity(0.3), lineWidth: 1)
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel(accessibilityBannerLabel)
    }

    // MARK: - Subviews

    private var headerRow: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: "person.badge.key.fill")
                .foregroundStyle(.orange)
                .font(.subheadline.weight(.semibold))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text("Process identity needs review")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.primary)
                    .accessibilityAddTraits(.isHeader)
                Text(providerName)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var reasonRow: some View {
        Text(reasonDetail ?? defaultReasonText)
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityLabel("Reason: \(reasonDetail ?? defaultReasonText)")
    }

    private var actionRow: some View {
        HStack(spacing: 8) {
            copyDiagnosticButton
            retryIdentityCheckButton
            markProcessAbsentButton
            openEvidenceButton
        }
        .buttonStyle(.plain)
    }

    private var copyDiagnosticButton: some View {
        Button(action: copyDiagnostic) {
            Label(
                copyConfirmed ? "Copied" : "Copy Diagnostic",
                systemImage: copyConfirmed ? "checkmark" : "doc.on.clipboard"
            )
            .font(.caption.weight(.medium))
            .foregroundStyle(copyConfirmed ? Color.green : Color.accentColor)
        }
        .accessibilityLabel(copyConfirmed ? "Diagnostic copied" : "Copy diagnostic to clipboard")
        .accessibilityHint("Copies the provider session diagnostic payload to the clipboard for sharing")
    }

    private var retryIdentityCheckButton: some View {
        Button("Retry Identity Check", action: onRetryIdentityCheck)
            .font(.caption.weight(.medium))
            .foregroundStyle(Color.accentColor)
            .accessibilityLabel("Retry identity check for \(providerName)")
            .accessibilityHint("Attempts to re-verify that the provider process identity matches the stored record")
    }

    private var markProcessAbsentButton: some View {
        Button("Mark Process Absent", action: onMarkProcessAbsent)
            .font(.caption.weight(.medium))
            .foregroundStyle(.red)
            .accessibilityLabel("Mark \(providerName) process as absent")
            .accessibilityHint("Records that the provider process is no longer running and clears the identity hold")
    }

    private var openEvidenceButton: some View {
        Button("Open Provider Session Evidence", action: onOpenProviderSessionEvidence)
            .font(.caption.weight(.medium))
            .foregroundStyle(Color.secondary)
            .accessibilityLabel("Open provider session evidence panel for \(providerName)")
            .accessibilityHint("Opens the full provider session evidence panel including process identity history")
    }

    private var disabledCommandNote: some View {
        HStack(spacing: 4) {
            Image(systemName: "exclamationmark.triangle")
                .foregroundStyle(.orange)
                .font(.caption2)
                .accessibilityHidden(true)
            Text("Automatic retry paused: process identity is ambiguous.")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Automatic retry paused: process identity is ambiguous.")
    }

    // MARK: - Actions

    private func copyDiagnostic() {
        let payload = defaultDiagnosticPayload
        #if canImport(AppKit)
        let pasteboard = NSPasteboard.general
        pasteboard.prepareForNewContents(with: .currentHostOnly)
        pasteboard.setString(payload, forType: .string)
        #endif
        withAnimation(.easeInOut(duration: 0.2)) {
            copyConfirmed = true
        }
        Task {
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            await MainActor.run {
                withAnimation { copyConfirmed = false }
            }
        }
    }

    // MARK: - Helpers

    private var defaultReasonText: String {
        "Forge could not prove this provider process is still the same process that was cancelled. Automatic retry is paused until you verify the process identity."
    }

    /// Diagnostic payload per manual_process_identity_check_ui_v1.available_actions.copy_diagnostic:
    /// Copies provider_session_id, cancellation_epoch, process_fate, last_seen_pid,
    /// process_start_identity hash, and latest receipt id with secrets redacted.
    private var defaultDiagnosticPayload: String {
        var lines = [
            "provider_session_id: \(sessionId)",
            "provider: \(providerName)",
            "process_fate: identity_ambiguous",
            "operator_next_step_code: manual_process_identity_check",
        ]
        if let epoch = cancellationEpoch {
            lines.append("cancellation_epoch: \(epoch)")
        }
        if let pid = lastSeenPid {
            lines.append("last_seen_pid: \(pid)")
        }
        if let hash = processStartIdentityHash {
            lines.append("process_start_identity_hash: \(hash)")
        } else {
            lines.append("process_start_identity_hash: <not_available>")
        }
        if let receiptId = latestReceiptId {
            lines.append("latest_receipt_id: \(receiptId)")
        } else {
            lines.append("latest_receipt_id: <not_available>")
        }
        return lines.joined(separator: "\n")
    }

    private var accessibilityBannerLabel: String {
        "Process identity review required for \(providerName). \(reasonDetail ?? defaultReasonText)"
    }
}

// MARK: - Preview

#if DEBUG
struct ManualProcessIdentityCheckBanner_Previews: PreviewProvider {
    static var previews: some View {
        VStack(spacing: 16) {
            ManualProcessIdentityCheckBanner(
                providerName: "codex",
                sessionId: "psess-preview-1",
                cancellationEpoch: 3,
                lastSeenPid: 12345,
                processStartIdentityHash: "abc123def456",
                latestReceiptId: "rcpt-preview-1",
                reasonDetail: "PID mismatch detected between stored identity and current OS process.",
                onRetryIdentityCheck: {},
                onMarkProcessAbsent: {},
                onOpenProviderSessionEvidence: {}
            )
            .padding()

            ManualProcessIdentityCheckBanner(
                providerName: "claude",
                sessionId: "psess-preview-2",
                cancellationEpoch: nil,
                lastSeenPid: nil,
                processStartIdentityHash: nil,
                latestReceiptId: nil,
                reasonDetail: nil,
                onRetryIdentityCheck: {},
                onMarkProcessAbsent: {},
                onOpenProviderSessionEvidence: {}
            )
            .padding()
        }
        .frame(width: 480)
    }
}
#endif
