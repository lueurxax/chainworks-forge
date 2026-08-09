import Foundation
import AppKit

/// P089: Protocol for writing a redacted temp artifact row to the pasteboard.
/// All copy paths — Edit > Copy, Command-C, toolbar Copy, context Copy, Copy Redacted Row —
/// route through this writer. Raw absolute paths never reach the pasteboard; only the
/// backend-redacted pathDisplay string is written.
@MainActor
protocol TempArtifactRowPasteboardWriting {
    func writeRedactedRow(_ row: TempArtifactInventoryResponse.Row, stale: Bool)
}

/// Production: writes one NSPasteboardType.string value to NSPasteboard.general.
/// Payload fields: path_display, path_hash, lifecycle_classification,
/// dry_run_recommendation (when present), generated_at, stale,
/// source_generated_at (when stale=true).
@MainActor
struct TempArtifactRowPasteboardWriter: TempArtifactRowPasteboardWriting {
    func writeRedactedRow(_ row: TempArtifactInventoryResponse.Row, stale: Bool) {
        let payload = buildPayload(row: row, stale: stale)
        // Approved proposal's pasteboard_protocol: clearContents() then write exactly
        // one NSPasteboard.PasteboardType.string value.
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(payload, forType: .string)
    }

    private func buildPayload(row: TempArtifactInventoryResponse.Row, stale: Bool) -> String {
        var parts: [String] = [
            "path_display: \(row.pathDisplay)",
            "path_hash: \(row.pathHash)",
            "lifecycle_classification: \(row.lifecycleClassification)",
            "generated_at: \(row.generatedAt)",
            "stale: \(stale)",
        ]
        if stale {
            // source_generated_at records when the stale row was originally produced.
            parts.append("source_generated_at: \(row.generatedAt)")
        }
        if let rec = row.dryRunRecommendation {
            parts.append("dry_run_recommendation: \(rec)")
        }
        return parts.joined(separator: "\n")
    }
}

/// Test spy: records calls without touching the system pasteboard.
@MainActor
final class TempArtifactRowPasteboardWriterSpy: TempArtifactRowPasteboardWriting {
    struct WriteCall {
        let row: TempArtifactInventoryResponse.Row
        let stale: Bool
    }

    private(set) var writeCalls: [WriteCall] = []

    func writeRedactedRow(_ row: TempArtifactInventoryResponse.Row, stale: Bool) {
        writeCalls.append(WriteCall(row: row, stale: stale))
    }
}
