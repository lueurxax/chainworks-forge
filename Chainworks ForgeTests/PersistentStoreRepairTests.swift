import Foundation
import SQLite3
import Testing
@testable import Chainworks_Forge

@Suite("PersistentStoreRepair", .tags(.fast))
struct PersistentStoreRepairTests {
    @Test("Repair adds missing AgentExecution ACP columns to legacy store")
    func repairAddsMissingACPColumns() throws {
        let tempURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension("sqlite")

        try createLegacyAgentExecutionTable(at: tempURL)
        try PersistentStoreRepairTestHarness.repairStore(at: tempURL)

        let columns = try fetchColumns(from: tempURL, tableName: "ZAGENTEXECUTION")
        #expect(columns.contains("ZACTUALADAPTERFAMILY"))
        #expect(columns.contains("ZACTUALCAPABILITYCLASS"))
    }

    private func createLegacyAgentExecutionTable(at url: URL) throws {
        var database: OpaquePointer?
        guard sqlite3_open_v2(url.path, &database, SQLITE_OPEN_CREATE | SQLITE_OPEN_READWRITE, nil) == SQLITE_OK else {
            defer { if database != nil { sqlite3_close(database) } }
            throw TestSQLiteError.openFailed
        }
        defer { sqlite3_close(database) }

        let statement = """
        CREATE TABLE ZAGENTEXECUTION (
            Z_PK INTEGER PRIMARY KEY,
            ZID BLOB,
            ZAGENTID TEXT
        );
        """

        guard sqlite3_exec(database, statement, nil, nil, nil) == SQLITE_OK else {
            throw TestSQLiteError.createFailed
        }
    }

    private func fetchColumns(from url: URL, tableName: String) throws -> Set<String> {
        var database: OpaquePointer?
        guard sqlite3_open_v2(url.path, &database, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else {
            defer { if database != nil { sqlite3_close(database) } }
            throw TestSQLiteError.openFailed
        }
        defer { sqlite3_close(database) }

        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, "PRAGMA table_info(\(tableName))", -1, &statement, nil) == SQLITE_OK else {
            throw TestSQLiteError.prepareFailed
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
}

enum PersistentStoreRepairTestHarness {
    static func repairStore(at url: URL) throws {
        try PersistentStoreRepair._repairStoreForTests(url)
    }
}

private enum TestSQLiteError: Error {
    case openFailed
    case createFailed
    case prepareFailed
}
