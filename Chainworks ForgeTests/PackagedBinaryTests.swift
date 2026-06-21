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

import Foundation
import Testing
@testable import Chainworks_Forge

struct PackagedBinaryTests {

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

    @Test func `Bundled daemon binary is present and executable`() throws {
        // Dev-workstation Debug runs of the test target do NOT install
        // the release daemon into `Contents/MacOS/` until the operator
        // has wired up the Embed Control-Plane Daemon build phase. In
        // that narrow case we cancel the test so the focused lane can run
        // green. In Release we promote the same absence to a hard failure:
        // the release bundle MUST ship the daemon (AC-9a / §7.2).
        guard let url = bundledDaemonURL() else {
            if Self.isReleaseLikeBuild {
                Issue.record(
                    "\(Self.binaryName) MUST be embedded in Release configurations (P042 AC-9a). Check the Embed Control-Plane Daemon build phase output."
                )
            } else {
                try Test.cancel(
                    "\(Self.binaryName) not embedded in this Debug configuration; run the release build or wire the Embed Control-Plane Daemon build phase"
                )
            }
            return
        }
        #expect(
            FileManager.default.isExecutableFile(atPath: url.path),
            "bundled daemon must be marked executable: \(url.path)"
        )
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        let size = (attrs[.size] as? Int64) ?? 0
        #expect(size > 0, "bundled daemon must be non-empty: \(url.path)")
    }

    /// P042 §7.6: when the binary is embedded, it must carry *some*
    /// signature — ad-hoc on dev builds, Developer ID on release. The
    /// release lane (`proposal-042-packaging`) enforces the authority
    /// comparison; this test just pins "signature present at all" so a
    /// stripped binary can't ship accidentally.
    @Test func `Bundled daemon binary carries some signature`() throws {
        guard let url = bundledDaemonURL() else {
            if Self.isReleaseLikeBuild {
                Issue.record(
                    "\(Self.binaryName) MUST be embedded + signed in Release configurations (P042 AC-9a)"
                )
            } else {
                try Test.cancel("\(Self.binaryName) not embedded in this Debug configuration")
            }
            return
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/codesign")
        proc.arguments = ["-dvv", url.path]
        let stderr = Pipe()
        proc.standardOutput = Pipe()
        proc.standardError = stderr
        try proc.run()
        proc.waitUntilExit()
        #expect(
            proc.terminationStatus == 0,
            "codesign -dvv must succeed on the embedded daemon"
        )
        // `codesign` emits metadata on stderr; a signed binary contains
        // either `Authority=...` or at minimum `Signature=adhoc`.
        let data = (try? stderr.fileHandleForReading.readToEnd()) ?? Data()
        let output = String(data: data, encoding: .utf8) ?? ""
        #expect(
            output.contains("Signature=") || output.contains("Authority="),
            "codesign output must mention a signature: \(output)"
        )
    }
}
