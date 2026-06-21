import Foundation

// ---------------------------------------------------------------------------
// P079 Readback Presenter
// Converts OutputContractRepairEvidence into operator-facing display state.
// Read-only; never authorizes output settlement, transition truth, or dispatch.
// ---------------------------------------------------------------------------

nonisolated struct OutputContractRepairPresentation: Sendable, Equatable {
    /// Compact badge label for run-row display.
    let compactSignalLabel: String
    /// Extended status label for inspector header.
    let statusLabel: String
    /// Presentation category binding for color/icon selection.
    let category: PresentationCategory
    /// Whether to show an indeterminate ProgressView chip (in_progress / prompt_sent).
    let showProgressChip: Bool
    /// Whether projection is stale (Stale UX — macos-r3-005).
    let isStale: Bool
    /// Relative timestamp for stale indicator.
    let staleSinceLabel: String?
    /// Diagnostic rows for inspector detail.
    let diagnosticRows: [String]
    /// Evidence artifact path for Reveal in Finder (nil if not under meta-root).
    let evidenceArtifactPath: String?
    /// Plan evidence paths (meta-root-relative only).
    let planEvidencePaths: [String]
    /// Permission decisions for accessibility rows.
    let permissionDecisions: [OutputContractPermissionDecision]
    /// Raw repair attempt ID for copy-identifier surface.
    let repairAttemptId: String
}

nonisolated enum OutputContractRepairPresenter {
    /// Returns nil for pre-P079 runs and feature-disabled runs (nil evidence).
    static func presentationIfPresent(
        for evidence: OutputContractRepairEvidence?
    ) -> OutputContractRepairPresentation? {
        guard let evidence else { return nil }
        return presentation(for: evidence)
    }

    static func presentation(
        for evidence: OutputContractRepairEvidence
    ) -> OutputContractRepairPresentation {
        let category = evidence.status.presentationCategory
        let showProgressChip = evidence.status == .inProgress &&
            (evidence.lease?.state == .reserved || evidence.lease?.state == .promptSent)
        let isStale = evidence.projectionIntegrity == "stale" ||
            evidence.projectionIntegrity == "permanently_stale"
        let staleSinceLabel: String? = isStale
            ? staleSinceDisplayLabel(evidence.projectionStaleSince)
            : nil

        var diagnosticRows: [String] = []
        diagnosticRows.append("Failure: \(evidence.initialFailureClass.rawValue)")
        if let subtype = evidence.initialFailureSubtype {
            diagnosticRows.append("Subtype: \(subtype)")
        }
        if let repair = evidence.sameSessionRepair {
            diagnosticRows.append("Repair: \(repair.result.rawValue)")
        }
        if let recovery = evidence.transcriptRecovery {
            diagnosticRows.append("Recovery: \(recovery.result.rawValue)")
        }
        if let fallback = evidence.providerFallback {
            diagnosticRows.append("Fallback: \(fallback.result.rawValue)")
        }
        if let settlement = evidence.finalOutputSettlement {
            diagnosticRows.append("Settlement: \(settlement)")
        }
        if evidence.projectionIntegrity == "stale" {
            diagnosticRows.append("Stale · retrying…")
        } else if evidence.projectionIntegrity == "permanently_stale" {
            diagnosticRows.append("Projection permanently stale — manual investigation required")
        }

        let compactLabel = compactSignalLabel(for: evidence, category: category)
        let statusLabel = inspectorStatusLabel(for: evidence, category: category)

        return OutputContractRepairPresentation(
            compactSignalLabel: compactLabel,
            statusLabel: statusLabel,
            category: category,
            showProgressChip: showProgressChip,
            isStale: isStale,
            staleSinceLabel: staleSinceLabel,
            diagnosticRows: diagnosticRows,
            evidenceArtifactPath: Self.safeRelativePath(evidence.evidenceArtifactPath),
            planEvidencePaths: (evidence.providerPlanEvidence?.paths ?? []).filter { Self.safeRelativePath($0) != nil },
            permissionDecisions: evidence.permissionDecisions,
            repairAttemptId: evidence.repairAttemptId
        )
    }

    // P079-SEC-LOW-001: reject absolute paths, path-traversal sequences, backslash
    // traversal forms, and URL-encoded traversal before exposing evidence paths to
    // Reveal in Finder or inspector surfaces. This is defense-in-depth against future
    // sources that may bypass server-side sanitization.
    static func safeRelativePath(_ path: String?) -> String? {
        guard let path else { return nil }
        // Reject absolute paths.
        guard !path.hasPrefix("/") else { return nil }
        // Reject backslash-based traversal (Windows-style or escaped).
        guard !path.contains("\\") else { return nil }
        // Reject URL-encoded traversal sequences (%2e%2e, %2f, %5c, etc.).
        // %5c is URL-encoded backslash; mirroring server-side behavior for defense in depth.
        let lower = path.lowercased()
        guard !lower.contains("%2e%2e"), !lower.contains("%2f"), !lower.contains("%5c") else { return nil }
        // Reject '..' components at any position in the path.
        let components = path.components(separatedBy: "/")
        guard !components.contains("..") else { return nil }
        return path
    }

    private static func compactSignalLabel(
        for evidence: OutputContractRepairEvidence,
        category: PresentationCategory
    ) -> String {
        switch category {
        case .informational:
            return evidence.status == .inProgress
                ? "Contract Repair: in progress"
                : "Contract Repair: not attempted"
        case .recovered:
            return "Contract Repair: recovered"
        case .blocked:
            return "Contract Repair: blocked"
        case .skipped:
            return "Contract Repair: skipped"
        case .failed:
            return "Contract Repair: failed"
        case .cancelled:
            return "Contract Repair: cancelled"
        case .unknownDiagnostic(_):
            return "Contract Repair: unknown"
        }
    }

    private static func inspectorStatusLabel(
        for evidence: OutputContractRepairEvidence,
        category: PresentationCategory
    ) -> String {
        switch category {
        case .informational:
            return evidence.status == .inProgress
                ? "Repair in progress"
                : "No repair attempted"
        case .recovered:
            return "Output recovered"
        case .blocked:
            return "Contract blocked — operator action required"
        case .skipped:
            return "Repair skipped"
        case .failed:
            return "Repair failed"
        case .cancelled:
            return "Repair cancelled"
        case .unknownDiagnostic(_):
            return "Unknown repair state"
        }
    }

    private static func staleSinceDisplayLabel(_ rawValue: String?) -> String? {
        guard let rawValue, !rawValue.isEmpty else { return nil }
        let parser = ISO8601DateFormatter()
        parser.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let parsed = parser.date(from: rawValue) ?? {
            let fallback = ISO8601DateFormatter()
            fallback.formatOptions = [.withInternetDateTime]
            return fallback.date(from: rawValue)
        }()
        guard let parsed else { return rawValue }

        let relative = RelativeDateTimeFormatter()
        relative.unitsStyle = .full
        let relativeLabel = relative.localizedString(for: parsed, relativeTo: Date())
        return "\(relativeLabel) · \(rawValue)"
    }
}

// MARK: - Presentation category display helpers (macos-r3-001, ui-001..004)

extension PresentationCategory {
    /// SF Symbol name for this category (accessibility contract).
    var sfSymbolName: String {
        switch self {
        case .informational: return "info.circle"
        case .recovered: return "checkmark.shield"
        case .blocked: return "exclamationmark.octagon"
        case .skipped: return "slash.circle"
        case .failed: return "xmark.octagon"
        case .cancelled: return "xmark.circle"
        case .unknownDiagnostic(_): return "questionmark.diamond"
        }
    }

    /// Accessibility label combining category and status context.
    func accessibilityLabel(status: OutputContractRepairStatus) -> String {
        switch self {
        case .informational: return "Contract repair informational: \(status.rawValue)"
        case .recovered: return "Contract repair recovered"
        case .blocked: return "Contract repair blocked: operator action required"
        case .skipped: return "Contract repair skipped"
        case .failed: return "Contract repair failed"
        case .cancelled: return "Contract repair cancelled"
        case .unknownDiagnostic(_): return "Contract repair unknown diagnostic: \(status.rawValue)"
        }
    }

    /// Stable accessibility identifier per badge for UI test selection.
    var accessibilityIdentifier: String {
        "p079-repair-badge-\(rawValue)"
    }
}
