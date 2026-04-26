import Foundation
import OSLog

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
    private nonisolated static let defaultsKey = "securityScopedBookmarks.v1"
    private nonisolated static let logger = Logger(subsystem: "xax.Chainworks-Forge", category: "App")
    #if DEBUG
    private nonisolated(unsafe) static var bookmarkDataProviderForTesting: ((URL) throws -> Data)? = nil
    #endif

    @discardableResult
    nonisolated static func remember(path: String?, kind: SecurityScopedBookmarkKind) -> Bool {
        guard let path, !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return false }
        return remember(url: URL(fileURLWithPath: path), kind: kind)
    }

    @discardableResult
    nonisolated static func remember(url: URL, kind: SecurityScopedBookmarkKind) -> Bool {
        let standardized = url.standardizedFileURL
        guard standardized.isFileURL, !standardized.path.isEmpty else { return false }

        let bookmarkData: Data
        do {
            #if DEBUG
            if let bookmarkDataProviderForTesting {
                bookmarkData = try bookmarkDataProviderForTesting(standardized)
            } else {
                bookmarkData = try standardized.bookmarkData(
                    options: .withSecurityScope,
                    includingResourceValuesForKeys: nil,
                    relativeTo: nil
                )
            }
            #else
            bookmarkData = try standardized.bookmarkData(
                options: .withSecurityScope,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
            #endif
        } catch {
            logError("Failed to create security-scoped bookmark for \(standardized.path): \(error.localizedDescription)")
            return false
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
        return saveRecords(records)
    }

    nonisolated static func rememberConfiguredPaths(in configuration: AppConfiguration) {
        remember(path: configuration.workflowSourcePath, kind: .workflowSource)
        remember(path: configuration.agentCatalogSourcePath, kind: .catalogSource)
        remember(path: configuration.supportBundleExportPath, kind: .supportBundleRoot)
    }

    nonisolated static func withAccess<T>(to url: URL, perform: (URL) throws -> T) rethrows -> T {
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

    nonisolated static func loadData(from url: URL) throws -> Data {
        do {
            return try withAccess(to: url) { try Data(contentsOf: $0) }
        } catch {
            let originalURL = url.standardizedFileURL
            if FileManager.default.isReadableFile(atPath: originalURL.path) {
                return try Data(contentsOf: originalURL)
            }
            throw error
        }
    }

    nonisolated static func loadString(from url: URL, encoding: String.Encoding = .utf8) throws -> String {
        let data = try loadData(from: url)
        guard let string = String(data: data, encoding: encoding) else {
            throw CocoaError(.fileReadInapplicableStringEncoding)
        }
        return string
    }

    nonisolated static func fileExists(at url: URL) -> Bool {
        let existsWithAccess = withAccess(to: url) { securedURL in
            FileManager.default.fileExists(atPath: securedURL.path)
        }
        if existsWithAccess {
            return true
        }
        return FileManager.default.fileExists(atPath: url.standardizedFileURL.path)
    }

    nonisolated static func itemStatus(atPath path: String) -> (exists: Bool, isDirectory: Bool) {
        let url = URL(fileURLWithPath: path)
        return withAccess(to: url) { scopedURL in
            var isDirectory: ObjCBool = false
            let exists = FileManager.default.fileExists(atPath: scopedURL.path, isDirectory: &isDirectory)
            return (exists, isDirectory.boolValue)
        }
    }

    nonisolated static func hasBookmark(for url: URL) -> Bool {
        nearestRecord(for: url) != nil
    }

    nonisolated static func authorizedRepositoryRoots() -> [URL] {
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

    private nonisolated static func resolvedURL(for requestedURL: URL) -> URL? {
        if shouldBypassBookmarkResolution(for: requestedURL) {
            removeRecordsMatching(path: requestedURL.standardizedFileURL.path)
            return nil
        }

        guard let record = nearestRecord(for: requestedURL) else { return nil }
        var isStale = false
        guard let resolved = try? URL(
            resolvingBookmarkData: record.bookmarkData,
            options: [.withSecurityScope, .withoutUI],
            relativeTo: nil,
            bookmarkDataIsStale: &isStale
        ) else {
            if shouldQuietlyDiscardBookmarkFailure(for: requestedURL) {
                removeRecordsMatching(path: record.path)
            } else {
                logError("Failed to resolve security-scoped bookmark for \(requestedURL.standardizedFileURL.path)")
            }
            return nil
        }

        if isStale {
            remember(url: resolved, kind: record.kind)
        }

        return resolved
    }

    private nonisolated static func nearestRecord(for requestedURL: URL) -> SecurityScopedBookmarkRecord? {
        let requestedPath = requestedURL.standardizedFileURL.path
        return loadRecords()
            .filter { requestedPath == $0.path || requestedPath.hasPrefix($0.path + "/") }
            .max { $0.path.count < $1.path.count }
    }

    private nonisolated static func isDirectoryBookmarkKind(_ kind: SecurityScopedBookmarkKind) -> Bool {
        switch kind {
        case .workspaceRoot, .supportBundleRoot, .artifactRoot:
            return true
        case .workflowSource, .catalogSource, .settingsFile:
            return false
        }
    }

    private nonisolated static func shouldBypassBookmarkResolution(for requestedURL: URL) -> Bool {
        isTemporaryExampleFixturePath(requestedURL.standardizedFileURL.path)
    }

    private nonisolated static func shouldQuietlyDiscardBookmarkFailure(for requestedURL: URL) -> Bool {
        let requestedPath = requestedURL.standardizedFileURL.path
        if isTemporaryExampleFixturePath(requestedPath) {
            return true
        }
        return false
    }

    private nonisolated static func isTemporaryPath(_ path: String) -> Bool {
        let tempRoot = FileManager.default.temporaryDirectory.standardizedFileURL.path
        return path == tempRoot || path.hasPrefix(tempRoot + "/")
    }

    private nonisolated static func isTemporaryExampleFixturePath(_ path: String) -> Bool {
        guard isTemporaryPath(path) else { return false }
        return path.contains("/examples/")
    }

    private nonisolated static func removeRecordsMatching(path: String) {
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        let records = loadRecords()
        let filtered = records.filter { $0.path != standardizedPath }
        guard filtered.count != records.count else { return }
        _ = saveRecords(filtered)
    }

    private nonisolated static func loadRecords() -> [SecurityScopedBookmarkRecord] {
        guard let data = UserDefaults.standard.data(forKey: defaultsKey) else {
            return []
        }
        guard let records = try? JSONDecoder().decode([SecurityScopedBookmarkRecord].self, from: data) else {
            logError("Security-scoped bookmark store could not be decoded; ignoring persisted bookmarks")
            return []
        }
        return records
    }

    @discardableResult
    private nonisolated static func saveRecords(_ records: [SecurityScopedBookmarkRecord]) -> Bool {
        guard let data = try? JSONEncoder().encode(records) else {
            logError("Security-scoped bookmark store could not be encoded")
            return false
        }
        UserDefaults.standard.set(data, forKey: defaultsKey)
        return true
    }

    private nonisolated static func logError(_ message: String) {
        logger.error("\(message, privacy: .public)")
    }

#if DEBUG
    nonisolated static func resetForTesting() {
        UserDefaults.standard.removeObject(forKey: defaultsKey)
        bookmarkDataProviderForTesting = nil
    }

    nonisolated static func bookmarkedPathsForTesting() -> [String] {
        loadRecords().map(\.path).sorted()
    }

    nonisolated static func setRawBookmarkStoreDataForTesting(_ data: Data?) {
        UserDefaults.standard.set(data, forKey: defaultsKey)
    }

    nonisolated static func installBookmarkDataProviderForTesting(_ provider: ((URL) throws -> Data)?) {
        bookmarkDataProviderForTesting = provider
    }
#endif
}
