import Foundation

// MARK: - Proposal 013 Layer P: Proposal Draft Compaction Policy

/// Applies bounded output-size discipline to proposal drafting and stores
/// truncation/compaction metadata when invoked.
///
/// Required persisted metadata (§8.2):
/// - original output size
/// - compacted output size
/// - compaction strategy
/// - whether the stage succeeded with compaction or failed despite compaction
struct ProposalDraftCompactionPolicy {

    /// Maximum output size in bytes before compaction is triggered.
    /// Default: 256KB — large enough for detailed proposals, small enough to prevent pathological artifacts.
    static let defaultMaxOutputSize: Int = 256 * 1024

    /// Apply compaction if the output exceeds the limit.
    /// Returns the original data if within bounds, or compacted data with metadata.
    static func apply(
        outputName: String,
        data: Data,
        maxSize: Int = defaultMaxOutputSize
    ) -> CompactionResult {
        guard data.count > maxSize else {
            return CompactionResult(
                data: data,
                metadata: nil,
                wasCompacted: false
            )
        }

        // Strategy: truncate to max size with ellipsis marker
        let strategy: CompactionStrategy = .truncateWithMarker
        let truncated = compactData(data, maxSize: maxSize, strategy: strategy)

        let metadata = CompactionMetadata(
            outputName: outputName,
            originalSize: data.count,
            compactedSize: truncated.count,
            strategy: strategy,
            timestamp: Date()
        )

        return CompactionResult(
            data: truncated,
            metadata: metadata,
            wasCompacted: true
        )
    }

    // MARK: - Internal Compaction

    private static func compactData(
        _ data: Data,
        maxSize: Int,
        strategy: CompactionStrategy
    ) -> Data {
        switch strategy {
        case .truncateWithMarker:
            guard let text = String(data: data, encoding: .utf8) else {
                // Binary data: truncate directly
                return data.prefix(maxSize)
            }
            let marker = "\n\n---\n\n[Compacted: original size \(data.count) bytes, truncated to \(maxSize) bytes by ProposalDraftCompactionPolicy]\n"
            let allowedTextSize = maxSize - marker.utf8.count
            if allowedTextSize <= 0 {
                return Data(marker.utf8)
            }
            let truncatedText = String(text.prefix(allowedTextSize))
            return Data((truncatedText + marker).utf8)
        }
    }
}

// MARK: - Compaction Result

struct CompactionResult: Sendable {
    let data: Data
    let metadata: CompactionMetadata?
    let wasCompacted: Bool
}

// MARK: - Compaction Metadata (§8.2)

struct CompactionMetadata: Codable, Sendable {
    let outputName: String
    let originalSize: Int
    let compactedSize: Int
    let strategy: CompactionStrategy
    let timestamp: Date
    /// Proposal 013 §8.2: Outcome truth — whether the stage succeeded with compaction
    /// or failed despite compaction. Set after stage settlement.
    var stageOutcome: CompactionOutcome?
}

/// Proposal 013 §8.2: Required compaction outcome truth.
enum CompactionOutcome: String, Codable, Sendable {
    case succeededWithCompaction = "succeeded_with_compaction"
    case failedDespiteCompaction = "failed_despite_compaction"
}

// MARK: - Compaction Strategy

enum CompactionStrategy: String, Codable, Sendable {
    case truncateWithMarker = "truncate_with_marker"
}
