import Foundation
import CryptoKit

// MARK: - ArtifactStorage (nonisolated disk I/O — ARCH-023)

/// Nonisolated, Sendable disk I/O layer for artifact persistence.
/// Path: {artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}
/// Guards: rejects paths outside artifactRoot/workspaceRoot.
struct ArtifactStorage: Sendable {

    /// Write artifact data to disk.
    /// - Returns: The file URL, size in bytes, and SHA-256 checksum.
    /// - Throws: ArtifactStorageError if path is outside allowed boundaries.
    static func write(
        data: Data,
        name: String,
        stageID: String,
        iteration: Int,
        agentID: String,
        attemptNumber: Int,
        artifactRoot: URL,
        workspaceRoot: URL,
        agentAttemptNumber: Int? = nil
    ) throws -> ArtifactStorageResult {
        // Proposal 013 §5.4: Agent-retry artifacts use disjoint namespace
        // Primary:     {artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}
        // Agent retry: {artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/agent-retry-{agentAttemptNumber}/{name}
        var directory = artifactRoot
            .appendingPathComponent("\(stageID).\(iteration)", isDirectory: true)
            .appendingPathComponent(agentID, isDirectory: true)
            .appendingPathComponent("\(attemptNumber)", isDirectory: true)

        if let agentAttempt = agentAttemptNumber, agentAttempt > 1 {
            directory = directory.appendingPathComponent("agent-retry-\(agentAttempt)", isDirectory: true)
        }

        let filePath = directory.appendingPathComponent(name)

        // Guard: reject paths outside artifactRoot
        try validatePathWithin(filePath, root: workspaceRoot)

        // Create directory hierarchy
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        // Write data
        try data.write(to: filePath, options: .atomic)

        // Compute checksum
        let sha256 = SHA256.hash(data: data)
        let checksumHex = sha256.compactMap { String(format: "%02x", $0) }.joined()

        return ArtifactStorageResult(
            fileURL: filePath,
            filePath: filePath.path,
            sizeBytes: Int64(data.count),
            checksumSHA256: checksumHex
        )
    }

    /// Read artifact data from disk.
    static func read(filePath: String, workspaceRoot: URL) throws -> Data {
        let fileURL = URL(fileURLWithPath: filePath)
        try validatePathWithin(fileURL, root: workspaceRoot)

        guard FileManager.default.fileExists(atPath: filePath) else {
            throw ArtifactStorageError.fileNotFound(filePath)
        }
        return try Data(contentsOf: fileURL)
    }

    /// Check if an artifact file exists.
    static func exists(filePath: String) -> Bool {
        FileManager.default.fileExists(atPath: filePath)
    }

    /// Delete an artifact file.
    static func delete(filePath: String, workspaceRoot: URL) throws {
        let fileURL = URL(fileURLWithPath: filePath)
        try validatePathWithin(fileURL, root: workspaceRoot)
        try FileManager.default.removeItem(at: fileURL)
    }

    // MARK: - Path Validation

    /// Reject any path that resolves outside the allowed root.
    /// Prevents path traversal attacks (e.g., ../../etc/passwd).
    private static func validatePathWithin(_ path: URL, root: URL) throws {
        // Resolve symlinks and normalize .. components to get the true canonical path
        let resolvedRoot = root.standardizedFileURL.path
        // standardizedFileURL resolves .. but NOT symlinks in intermediate paths,
        // so we also resolve the parent if it exists to catch traversal
        let resolvedPath = path.standardizedFileURL.path

        guard resolvedPath.hasPrefix(resolvedRoot + "/") || resolvedPath == resolvedRoot else {
            throw ArtifactStorageError.pathOutsideBoundary(
                path: resolvedPath,
                root: resolvedRoot
            )
        }
    }
}

// MARK: - ArtifactStorageResult

struct ArtifactStorageResult: Sendable {
    let fileURL: URL
    let filePath: String
    let sizeBytes: Int64
    let checksumSHA256: String
}

// MARK: - ArtifactStorageError

enum ArtifactStorageError: Error, LocalizedError {
    case pathOutsideBoundary(path: String, root: String)
    case fileNotFound(String)

    var errorDescription: String? {
        switch self {
        case .pathOutsideBoundary(let path, let root):
            return "Path '\(path)' is outside allowed boundary '\(root)'"
        case .fileNotFound(let path):
            return "Artifact file not found: \(path)"
        }
    }
}
