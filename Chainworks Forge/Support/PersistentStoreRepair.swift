import Foundation
import SQLite3

enum PersistentStoreRepair {
    private struct RequiredColumn {
        let name: String
        let sqlType: String
    }

    private static let agentExecutionColumns = [
        RequiredColumn(name: "ZACTUALADAPTERFAMILY", sqlType: "TEXT"),
        RequiredColumn(name: "ZACTUALCAPABILITYCLASS", sqlType: "TEXT"),
        RequiredColumn(name: "ZRUNTIMEPROFILEID", sqlType: "TEXT")
    ]
    private static let storeDirectoryName = "Chainworks Forge"
    private static let storeFileName = "default.store"

    static func repairDefaultStoreIfNeeded(isStoredInMemoryOnly: Bool) {
        guard !isStoredInMemoryOnly else { return }

        let storeURL: URL
        do {
            storeURL = try preparePersistentStoreIfNeeded()
        } catch {
            ForgeLogger.app.error("Persistent store preparation failed: \(error.localizedDescription)")
            return
        }

        guard FileManager.default.fileExists(atPath: storeURL.path) else { return }

        do {
            try ensureColumnsExist(
                in: storeURL,
                tableName: "ZAGENTEXECUTION",
                requiredColumns: agentExecutionColumns
            )
        } catch {
            ForgeLogger.app.error("Persistent store repair failed for \(storeURL.path): \(error.localizedDescription)")
        }
    }

#if DEBUG
    static func _repairStoreForTests(_ storeURL: URL) throws {
        try ensureColumnsExist(
            in: storeURL,
            tableName: "ZAGENTEXECUTION",
            requiredColumns: agentExecutionColumns
        )
    }

    static func _prepareStoreForTests(applicationSupportURL: URL) throws -> URL {
        try preparePersistentStoreIfNeeded(applicationSupportURL: applicationSupportURL)
    }

    static func _canonicalStoreURLForTests(applicationSupportURL: URL) -> URL {
        canonicalStoreURL(applicationSupportURL: applicationSupportURL)
    }
#endif

    static func canonicalStoreURL() -> URL {
        canonicalStoreURL(applicationSupportURL: applicationSupportDirectory())
    }

    private static func applicationSupportDirectory() -> URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support", isDirectory: true)
    }

    private static func canonicalStoreURL(applicationSupportURL: URL) -> URL {
        applicationSupportURL
            .appendingPathComponent(storeDirectoryName, isDirectory: true)
            .appendingPathComponent(storeFileName)
    }

    private static func legacyStoreURL(applicationSupportURL: URL) -> URL {
        applicationSupportURL.appendingPathComponent(storeFileName)
    }

    @discardableResult
    private static func preparePersistentStoreIfNeeded(applicationSupportURL: URL = applicationSupportDirectory()) throws -> URL {
        let fileManager = FileManager.default
        let canonicalURL = canonicalStoreURL(applicationSupportURL: applicationSupportURL)
        let legacyURL = legacyStoreURL(applicationSupportURL: applicationSupportURL)
        let canonicalDirectory = canonicalURL.deletingLastPathComponent()

        try fileManager.createDirectory(at: canonicalDirectory, withIntermediateDirectories: true)
        try migrateLegacyStoreIfNeeded(from: legacyURL, to: canonicalURL)
        return canonicalURL
    }

    private static func migrateLegacyStoreIfNeeded(from legacyURL: URL, to canonicalURL: URL) throws {
        let fileManager = FileManager.default
        let canonicalExists = fileManager.fileExists(atPath: canonicalURL.path)
        let legacyExists = fileManager.fileExists(atPath: legacyURL.path)

        let canonicalIsUsable = canonicalExists && fileSize(at: canonicalURL) > 0
        let legacyIsUsable = legacyExists && fileSize(at: legacyURL) > 0

        guard legacyIsUsable else {
            return
        }

        guard !canonicalIsUsable else {
            return
        }

        if canonicalExists {
            try removeStoreTripletIfPresent(at: canonicalURL)
        }

        try copyStoreTriplet(from: legacyURL, to: canonicalURL)
    }

    private static func copyStoreTriplet(from sourceURL: URL, to destinationURL: URL) throws {
        let fileManager = FileManager.default
        for (source, destination) in pairedStoreSidecars(sourceURL: sourceURL, destinationURL: destinationURL) {
            guard fileManager.fileExists(atPath: source.path) else { continue }
            if fileManager.fileExists(atPath: destination.path) {
                try fileManager.removeItem(at: destination)
            }
            try fileManager.copyItem(at: source, to: destination)
        }
    }

    private static func removeStoreTripletIfPresent(at storeURL: URL) throws {
        let fileManager = FileManager.default
        for url in [storeURL, sidecarURL(for: storeURL, suffix: "-wal"), sidecarURL(for: storeURL, suffix: "-shm")] {
            if fileManager.fileExists(atPath: url.path) {
                try fileManager.removeItem(at: url)
            }
        }
    }

    private static func pairedStoreSidecars(sourceURL: URL, destinationURL: URL) -> [(URL, URL)] {
        [
            (sourceURL, destinationURL),
            (sidecarURL(for: sourceURL, suffix: "-wal"), sidecarURL(for: destinationURL, suffix: "-wal")),
            (sidecarURL(for: sourceURL, suffix: "-shm"), sidecarURL(for: destinationURL, suffix: "-shm"))
        ]
    }

    private static func fileSize(at url: URL) -> Int64 {
        let values = try? url.resourceValues(forKeys: [.fileSizeKey])
        return Int64(values?.fileSize ?? 0)
    }

    private static func sidecarURL(for storeURL: URL, suffix: String) -> URL {
        URL(fileURLWithPath: storeURL.path + suffix)
    }

    private static func ensureColumnsExist(
        in storeURL: URL,
        tableName: String,
        requiredColumns: [RequiredColumn]
    ) throws {
        var database: OpaquePointer?
        guard sqlite3_open_v2(storeURL.path, &database, SQLITE_OPEN_READWRITE, nil) == SQLITE_OK else {
            defer { if database != nil { sqlite3_close(database) } }
            throw SQLiteRepairError.openFailed(message: sqliteErrorMessage(database))
        }
        defer { sqlite3_close(database) }

        let existingColumns = try fetchColumnNames(in: database, tableName: tableName)
        guard !existingColumns.isEmpty else { return }

        for column in requiredColumns where !existingColumns.contains(column.name) {
            let statement = "ALTER TABLE \(tableName) ADD COLUMN \(column.name) \(column.sqlType)"
            try execute(statement: statement, in: database)
            ForgeLogger.app.info("Persistent store repair added \(column.name) to \(tableName)")
        }
    }

    private static func fetchColumnNames(in database: OpaquePointer?, tableName: String) throws -> Set<String> {
        var statement: OpaquePointer?
        let pragma = "PRAGMA table_info(\(tableName))"
        guard sqlite3_prepare_v2(database, pragma, -1, &statement, nil) == SQLITE_OK else {
            throw SQLiteRepairError.prepareFailed(message: sqliteErrorMessage(database))
        }
        defer { sqlite3_finalize(statement) }

        var columns = Set<String>()
        while sqlite3_step(statement) == SQLITE_ROW {
            if let namePointer = sqlite3_column_text(statement, 1) {
                columns.insert(String(cString: namePointer))
            }
        }
        return columns
    }

    private static func execute(statement: String, in database: OpaquePointer?) throws {
        guard sqlite3_exec(database, statement, nil, nil, nil) == SQLITE_OK else {
            throw SQLiteRepairError.executionFailed(message: sqliteErrorMessage(database), statement: statement)
        }
    }

    private static func sqliteErrorMessage(_ database: OpaquePointer?) -> String {
        guard let database, let pointer = sqlite3_errmsg(database) else {
            return "Unknown SQLite error"
        }
        return String(cString: pointer)
    }

    private enum SQLiteRepairError: LocalizedError {
        case openFailed(message: String)
        case prepareFailed(message: String)
        case executionFailed(message: String, statement: String)

        var errorDescription: String? {
            switch self {
            case .openFailed(let message):
                return "Could not open SQLite store: \(message)"
            case .prepareFailed(let message):
                return "Could not inspect SQLite schema: \(message)"
            case .executionFailed(let message, let statement):
                return "Could not execute SQLite repair statement '\(statement)': \(message)"
            }
        }
    }
}
