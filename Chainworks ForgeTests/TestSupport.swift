import Foundation

/// Repository root resolved from this source file's path (two levels up from
/// `Chainworks ForgeTests/`). Model-free helper used by the surviving live tests.
func testRepositoryRootURL(file: StaticString = #filePath) -> URL {
    URL(fileURLWithPath: "\(file)", isDirectory: false)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
}
