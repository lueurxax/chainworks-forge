// P042 §7.2 / §7.5 / §7.6 packaged binary embedding tests.
//
// When the app bundle is built for release, the Xcode build phase
// `Embed Daemon Binary` copies `control-plane/target/release/control-plane`
// into the app's `Contents/MacOS/chainworks-forge-daemon`. The app
// itself launches that binary via `SMAppService.Agent`.
//
// This test suite proves two things about the bundle shape:
//
//   1. Presence — a bundled binary sits at the canonical path.
//   2. Executability — the file is marked `-x` so `posix_spawn` can
//      invoke it without a chmod dance.
//
// The signing-authority + notarization checks (AC-9b / Layer C) run on
// a release host via `./scripts/test-gate.sh proposal-042-packaging`;
// they are intentionally out of scope here because a dev workstation
// produces only ad-hoc signatures.

import XCTest
@testable import Chainworks_Forge

final class PackagedBinaryTests: XCTestCase {

    /// Path the embed phase writes to inside `Bundle.main`.
    private static let binaryName = "chainworks-forge-daemon"

    private func bundledDaemonURL() -> URL? {
        Bundle.main.url(forAuxiliaryExecutable: Self.binaryName)
    }

    /// Release / packaged configurations MUST contain the embedded
    /// daemon. Debug may omit it only if the developer hasn't set up
    /// the Embed Control-Plane Daemon build phase yet. We detect
    /// Release-or-release-like configuration through the scheme's
    /// `CONFIGURATION` build setting captured at build time. P042
    /// §7.6 forbids a skip-on-release: a stripped-release bundle
    /// must fail the gate, not silently skip.
    private static var isReleaseLikeBuild: Bool {
        #if DEBUG
        return false
        #else
        return true
        #endif
    }

    func test_bundled_daemon_binary_is_present_and_executable() throws {
        // Dev-workstation Debug runs of the XCTest target do NOT
        // install the release daemon into `Contents/MacOS/` until the
        // operator has wired up the Embed Control-Plane Daemon build
        // phase. In that narrow case we still emit an `XCTSkip` so the
        // focused lane can run green. In Release we promote the same
        // absence to a hard `XCTFail`: the release bundle MUST ship
        // the daemon (AC-9a / §7.2).
        guard let url = bundledDaemonURL() else {
            if Self.isReleaseLikeBuild {
                XCTFail(
                    "\(Self.binaryName) MUST be embedded in Release configurations (P042 AC-9a). "
                    + "Check the Embed Control-Plane Daemon build phase output."
                )
                return
            }
            throw XCTSkip(
                "\(Self.binaryName) not embedded in this Debug configuration; "
                + "run the release build or wire the Embed Control-Plane Daemon build phase"
            )
        }
        XCTAssertTrue(
            FileManager.default.isExecutableFile(atPath: url.path),
            "bundled daemon must be marked executable: \(url.path)"
        )
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        let size = (attrs[.size] as? Int64) ?? 0
        XCTAssertGreaterThan(size, 0, "bundled daemon must be non-empty: \(url.path)")
    }

    /// P042 §7.6: when the binary is embedded, it must carry *some*
    /// signature — ad-hoc on dev builds, Developer ID on release. The
    /// release lane (`proposal-042-packaging`) enforces the authority
    /// comparison; this test just pins "signature present at all" so a
    /// stripped binary can't ship accidentally.
    func test_bundled_daemon_binary_carries_some_signature() throws {
        guard let url = bundledDaemonURL() else {
            if Self.isReleaseLikeBuild {
                XCTFail(
                    "\(Self.binaryName) MUST be embedded + signed in Release configurations (P042 AC-9a)"
                )
                return
            }
            throw XCTSkip("\(Self.binaryName) not embedded in this Debug configuration")
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/codesign")
        proc.arguments = ["-dvv", url.path]
        let stderr = Pipe()
        proc.standardOutput = Pipe()
        proc.standardError = stderr
        try proc.run()
        proc.waitUntilExit()
        XCTAssertEqual(
            proc.terminationStatus,
            0,
            "codesign -dvv must succeed on the embedded daemon"
        )
        // `codesign` emits metadata on stderr; a signed binary contains
        // either `Authority=...` or at minimum `Signature=adhoc`.
        let data = (try? stderr.fileHandleForReading.readToEnd()) ?? Data()
        let output = String(data: data, encoding: .utf8) ?? ""
        XCTAssertTrue(
            output.contains("Signature=") || output.contains("Authority="),
            "codesign output must mention a signature: \(output)"
        )
    }
}
