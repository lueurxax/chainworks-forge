import Foundation

enum ProjectRootPolicyError: LocalizedError, Equatable {
    case missingRequiredProjectRoot
    case invalidProjectRoot(String)

    var errorDescription: String? {
        switch self {
        case .missingRequiredProjectRoot:
            return "Workflow requires project access but no effective project root is configured."
        case .invalidProjectRoot(let path):
            return "Project root path is not a valid accessible directory: \(path)"
        }
    }
}

enum ProjectRootPolicy {
    static func normalizePath(_ path: String?) -> String? {
        let trimmed = path?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else { return nil }
        return trimmed
    }

    static func effectiveProjectRoot(
        workspaceRootPath: String?,
        deliveryRepoRootPath: String?
    ) -> String? {
        normalizePath(workspaceRootPath) ?? normalizePath(deliveryRepoRootPath)
    }

    static func requireProjectRoot(
        workspaceRootPath: String?,
        deliveryRepoRootPath: String?
    ) throws -> String {
        guard let path = effectiveProjectRoot(
            workspaceRootPath: workspaceRootPath,
            deliveryRepoRootPath: deliveryRepoRootPath
        ) else {
            throw ProjectRootPolicyError.missingRequiredProjectRoot
        }
        return path
    }

    static func validateAccessibleProjectRoot(atPath path: String) throws {
        let status = SecurityScopedAccess.itemStatus(atPath: path)
        guard status.exists, status.isDirectory else {
            throw ProjectRootPolicyError.invalidProjectRoot(path)
        }
    }

    static func requireAccessibleProjectRoot(
        workspaceRootPath: String?,
        deliveryRepoRootPath: String?
    ) throws -> String {
        let path = try requireProjectRoot(
            workspaceRootPath: workspaceRootPath,
            deliveryRepoRootPath: deliveryRepoRootPath
        )
        try validateAccessibleProjectRoot(atPath: path)
        return path
    }
}
