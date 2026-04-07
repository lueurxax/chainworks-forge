import Foundation

enum SecurityScopedBookmarkKind: String, Codable, CaseIterable {
    case workspaceRoot
    case workflowSource
    case catalogSource
    case supportBundleRoot
    case settingsFile
    case artifactRoot
}

private struct SecurityScopedBookmarkRecord: Codable {
    let path: String
    let kind: SecurityScopedBookmarkKind
    let bookmarkData: Data
}

enum SecurityScopedAccess {
    private static let defaultsKey = "securityScopedBookmarks.v1"

    static func remember(path: String?, kind: SecurityScopedBookmarkKind) {
        guard let path, !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        remember(url: URL(fileURLWithPath: path), kind: kind)
    }

    static func remember(url: URL, kind: SecurityScopedBookmarkKind) {
        let standardized = url.standardizedFileURL
        guard standardized.isFileURL, !standardized.path.isEmpty else { return }
        guard let bookmarkData = try? standardized.bookmarkData(
            options: .withSecurityScope,
            includingResourceValuesForKeys: nil,
            relativeTo: nil
        ) else {
            return
        }

        var records = loadRecords()
        records.removeAll { $0.path == standardized.path }
        records.append(
            SecurityScopedBookmarkRecord(
                path: standardized.path,
                kind: kind,
                bookmarkData: bookmarkData
            )
        )
        saveRecords(records)
    }

    static func rememberConfiguredPaths(in configuration: AppConfiguration) {
        remember(path: configuration.workflowSourcePath, kind: .workflowSource)
        remember(path: configuration.agentCatalogSourcePath, kind: .catalogSource)
        remember(path: configuration.supportBundleExportPath, kind: .supportBundleRoot)
    }

    static func withAccess<T>(to url: URL, perform: (URL) throws -> T) rethrows -> T {
        guard let securedURL = resolvedURL(for: url) else {
            return try perform(url)
        }

        let didStart = securedURL.startAccessingSecurityScopedResource()
        defer {
            if didStart {
                securedURL.stopAccessingSecurityScopedResource()
            }
        }

        return try perform(securedURL)
    }

    static func loadData(from url: URL) throws -> Data {
        try withAccess(to: url) { try Data(contentsOf: $0) }
    }

    static func loadString(from url: URL, encoding: String.Encoding = .utf8) throws -> String {
        let data = try loadData(from: url)
        guard let string = String(data: data, encoding: encoding) else {
            throw CocoaError(.fileReadInapplicableStringEncoding)
        }
        return string
    }

    static func fileExists(at url: URL) -> Bool {
        (try? withAccess(to: url) { FileManager.default.fileExists(atPath: $0.path) }) ?? false
    }

    static func itemStatus(atPath path: String) -> (exists: Bool, isDirectory: Bool) {
        let url = URL(fileURLWithPath: path)
        return (try? withAccess(to: url) { scopedURL in
            var isDirectory: ObjCBool = false
            let exists = FileManager.default.fileExists(atPath: scopedURL.path, isDirectory: &isDirectory)
            return (exists, isDirectory.boolValue)
        }) ?? (false, false)
    }

    static func hasBookmark(for url: URL) -> Bool {
        nearestRecord(for: url) != nil
    }

    static func authorizedRepositoryRoots() -> [URL] {
        var seen: Set<String> = []
        var roots: [URL] = []

        for record in loadRecords() {
            let url = URL(fileURLWithPath: record.path, isDirectory: isDirectoryBookmarkKind(record.kind))
            let root = isDirectoryBookmarkKind(record.kind) ? url : url.deletingLastPathComponent()
            let standardized = root.standardizedFileURL
            guard !standardized.path.isEmpty, seen.insert(standardized.path).inserted else { continue }
            roots.append(standardized)
        }

        return roots
    }

    private static func resolvedURL(for requestedURL: URL) -> URL? {
        guard let record = nearestRecord(for: requestedURL) else { return nil }
        var isStale = false
        guard let resolved = try? URL(
            resolvingBookmarkData: record.bookmarkData,
            options: [.withSecurityScope, .withoutUI],
            relativeTo: nil,
            bookmarkDataIsStale: &isStale
        ) else {
            return nil
        }

        if isStale {
            remember(url: resolved, kind: record.kind)
        }

        return resolved
    }

    private static func nearestRecord(for requestedURL: URL) -> SecurityScopedBookmarkRecord? {
        let requestedPath = requestedURL.standardizedFileURL.path
        return loadRecords()
            .filter { requestedPath == $0.path || requestedPath.hasPrefix($0.path + "/") }
            .max { $0.path.count < $1.path.count }
    }

    private static func isDirectoryBookmarkKind(_ kind: SecurityScopedBookmarkKind) -> Bool {
        switch kind {
        case .workspaceRoot, .supportBundleRoot, .artifactRoot:
            return true
        case .workflowSource, .catalogSource, .settingsFile:
            return false
        }
    }

    private static func loadRecords() -> [SecurityScopedBookmarkRecord] {
        guard let data = UserDefaults.standard.data(forKey: defaultsKey),
              let records = try? JSONDecoder().decode([SecurityScopedBookmarkRecord].self, from: data) else {
            return []
        }
        return records
    }

    private static func saveRecords(_ records: [SecurityScopedBookmarkRecord]) {
        guard let data = try? JSONEncoder().encode(records) else { return }
        UserDefaults.standard.set(data, forKey: defaultsKey)
    }

#if DEBUG
    static func resetForTesting() {
        UserDefaults.standard.removeObject(forKey: defaultsKey)
    }

    static func bookmarkedPathsForTesting() -> [String] {
        loadRecords().map(\.path).sorted()
    }
#endif
}
