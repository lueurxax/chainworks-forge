import Testing

extension Tag {
    /// High-ROI unit tests for the `fast` CI gate.
    @Tag static var fast: Self

    /// UI smoke tests for the `ui-smoke` CI gate.
    /// Note: UI tests remain on XCTest; this tag is for unit-level smoke coverage only.
    @Tag static var smoke: Self

    /// Tests requiring external provider connectivity.
    @Tag static var integration: Self

    /// Tests requiring a running Goose server.
    @Tag static var live: Self

    /// Provider-specific tests (Proposal 006 scope).
    @Tag static var provider: Self
}
