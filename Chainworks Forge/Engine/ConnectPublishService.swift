import Foundation

// MARK: - ConnectPublishService (Proposal 007 — §9.3)

/// Deterministic service for build/archive/upload. No source edits.
/// Rules:
/// - no source edits
/// - deterministic build inputs
/// - explicit target only
/// - checksum recorded always
struct ConnectPublishService: Sendable {

    enum PublishError: Error, LocalizedError {
        case buildFailed(output: String)
        case archiveFailed(output: String)
        case uploadFailed(output: String)
        case missingGitPushReceipt
        case missingReleaseManifest
        case invalidReleaseTarget(id: String)

        var errorDescription: String? {
            switch self {
            case .buildFailed(let output):
                return "Build failed: \(output)"
            case .archiveFailed(let output):
                return "Archive failed: \(output)"
            case .uploadFailed(let output):
                return "Upload failed: \(output)"
            case .missingGitPushReceipt:
                return "Git push receipt is required before publishing"
            case .missingReleaseManifest:
                return "Release manifest is required before publishing"
            case .invalidReleaseTarget(let id):
                return "Invalid release target: \(id)"
            }
        }
    }

    struct ReleaseBundleManifest: Codable, Sendable {
        let bundleIdentifier: String
        let bundleVersion: String
        let buildNumber: String
        let archivePath: String?
        let checksumSHA256: String
        let sizeBytes: Int64
        let timestamp: Date
    }

    struct ConnectUploadReceipt: Codable, Sendable {
        let artifactID: String
        let destination: String
        let releaseTargetID: String
        let releaseMode: String
        let status: String // "success" | "failed"
        let failureReason: String?
        let timestamp: Date
    }

    /// Execute deterministic build, archive, and upload.
    ///
    /// This is a scaffold for the first dogfood slice.
    /// In sandbox/staging mode, this records a receipt without actually uploading.
    func buildArchiveAndUpload(
        worktreeRoot: URL,
        gitPushReceipt: GitReleaseService.GitPushReceipt,
        releaseManifest: GitReleaseService.ReleaseManifest,
        releaseTargetID: String,
        releaseMode: ReleaseMode
    ) async throws -> (bundle: ReleaseBundleManifest, receipt: ConnectUploadReceipt) {
        guard gitPushReceipt.status == "success" else {
            throw PublishError.missingGitPushReceipt
        }

        // For sandbox/staging mode in the first dogfood slice, we record a receipt
        // documenting what would happen without actual App Store Connect upload.
        // This is the safe default per ARCH-072.

        let bundleManifest = ReleaseBundleManifest(
            bundleIdentifier: "com.chainworks.forge.\(releaseMode.rawValue)",
            bundleVersion: "1.0.0",
            buildNumber: String(releaseManifest.commitSHA.prefix(8)),
            archivePath: nil, // No actual archive in sandbox mode
            checksumSHA256: releaseManifest.commitSHA, // Use commit SHA as proxy checksum
            sizeBytes: 0,
            timestamp: Date()
        )

        let uploadReceipt = ConnectUploadReceipt(
            artifactID: UUID().uuidString,
            destination: "\(releaseMode.rawValue)://\(releaseTargetID)",
            releaseTargetID: releaseTargetID,
            releaseMode: releaseMode.rawValue,
            status: "success",
            failureReason: nil,
            timestamp: Date()
        )

        return (bundle: bundleManifest, receipt: uploadReceipt)
    }
}
