import Foundation
import SQLite3

enum PersistentStoreRepair {
    private struct RequiredColumn {
        let name: String
        let sqlType: String
    }

    private static let agentExecutionColumns = [
        RequiredColumn(name: "ZACTUALADAPTERFAMILY", sqlType: "TEXT"),
        RequiredColumn(name: "ZACTUALCAPABILITYCLASS", sqlType: "TEXT")
    ]

    static func repairDefaultStoreIfNeeded(isStoredInMemoryOnly: Bool) {
        guard !isStoredInMemoryOnly else { return }

        let storeURL = defaultStoreURL()
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
#endif

    private static func defaultStoreURL() -> URL {
        let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Library/Application Support", isDirectory: true)
        return applicationSupport.appendingPathComponent("default.store")
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
