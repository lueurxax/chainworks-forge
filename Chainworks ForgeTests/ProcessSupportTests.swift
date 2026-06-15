import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("ProcessSupport", .tags(.fast))
@MainActor
struct ProcessSupportTests {
    @Test("Executable discovery searches fallback directories outside inherited PATH")
    func resolveExecutableFindsBinaryInAdditionalSearchDirectory() throws {
        let tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }

        let executableURL = tempDirectory.appendingPathComponent("fake-acp")
        try "#!/bin/sh\nexit 0\n".write(to: executableURL, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o755],
            ofItemAtPath: executableURL.path
        )

        let resolved = ProcessSupport.resolveExecutable(
            "fake-acp",
            basePath: "",
            additionalSearchDirectories: [tempDirectory.path]
        )

        #expect(resolved == executableURL.path)
    }
}
