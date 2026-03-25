# Proposal 009: Test Suite Modernization — Swift Testing Migration, Parameterization, and Infrastructure Upgrade

| Field | Value |
|---|---|
| Date | 2026-03-25 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | None (internal quality improvement, orthogonal to feature proposals) |
| Adjacent work | [008-mvp-hardening-and-sign-off.md](008-mvp-hardening-and-sign-off.md) (sign-off evidence includes test health) |
| Goal | Migrate 17 XCTest-based unit test files to Swift Testing, eliminate assertion and setup boilerplate through parameterized tests and struct-based suites, upgrade shared test infrastructure (2 helper files) to support both frameworks during the transition, and introduce Xcode Test Plans as the tag-driven gate mechanism. |

---

## 1. Context

The Chainworks Forge unit test target (`Chainworks ForgeTests/`) currently contains 20 Swift files:

| Category | Count | Files |
|---|---|---|
| Executable XCTest unit tests | 17 | `ArtifactManagerTests`, `ArtifactValidationTests`, `EndToEndTests`, `GooseAgentExecutorTests`, `GooseServerLiveIntegrationTests`, `GooseServerTransportTests`, `GooseSessionBridgeTests`, `GooseStreamEventMapperTests`, `LiveGooseConnectionProofTests`, `LiveProposalWorkflowTests`, `OrchestratorTests`, `ProviderPlatformTests`, `ResumeManagerTests`, `RunPlanCompilerTests`, `SimulatedAgentExecutorTests`, `TransitionEvaluatorTests`, `WorkspaceIsolationTests` |
| Executable Swift Testing unit tests | 1 | `Chainworks_ForgeTests` |
| Shared helper files (not executable tests) | 2 | `TestSupport`, `SharedMocks` |

A separate UI test target (`Chainworks ForgeUITests/`) contains 1 XCUITest file.

Total across both targets: 21 files (~9,100 lines).

The project already targets macOS 26.2, builds with Xcode 16+, and uses Swift 5.
Full Swift Testing support has been available since Xcode 16 (WWDC 2024).
Apple's current guidance is explicit:

> "Consider using Swift Testing for new unit test development and migrating existing tests.
> Continue to use XCTest for user interface tests and performance tests."

One file (`Chainworks_ForgeTests.swift`) already uses Swift Testing with `@Suite`, `@Test`, `#expect`, and `#require`.
The remaining 17 executable XCTest unit test files still use `XCTestCase`, `XCTAssert*`, `override func setUp()`, and `override func tearDown()`.
The 2 helper files (`TestSupport.swift`, `SharedMocks.swift`) provide shared infrastructure; they are not executable test files but must be upgraded to support Swift Testing APIs.

The test suite is architecturally sound:

- full async/await adoption (16/17 XCTest files),
- @MainActor isolation (14/17 XCTest files),
- protocol-based mocking,
- centralized test factories (`TestSupport.swift`),
- thread-safe shared mocks (`SharedMocks.swift`),
- custom domain assertions (`assertRunCompleted`, `assertArtifactExists`),
- layered CI gates (`test-gate.sh` with 6 levels, driven by `xcodebuild` + `-only-testing:` arrays).

The modernization target is framework adoption and boilerplate reduction, not architectural redesign.

### 1.0.1 Current CI tooling baseline

The project currently has:

- **no `Package.swift`** — builds exclusively through `Chainworks Forge.xcodeproj`;
- **no `.xctestplan`** — gate selection is done via hard-coded `-only-testing:` arrays in `test-gate.sh`;
- **no SwiftPM test invocation path** — `swift test` is not available.

This means any tag-based gate mechanism must work through `xcodebuild` and Xcode Test Plans, not `swift test`.

### 1.1 What this proposal is not

Proposal 009 is **not**:

- a rewrite of test logic or coverage expansion,
- a new test harness or third-party framework adoption,
- a CI pipeline redesign,
- a UI test migration (XCUITest remains on XCTest),
- or a performance test migration (Swift Testing has no `measure {}` equivalent).

It is a systematic framework-level upgrade that:

- replaces `XCTestCase` classes with `@Suite` structs (17 files),
- replaces `XCTAssert*` assertions with `#expect` / `#require`,
- eliminates duplicated test methods through parameterized tests,
- replaces `setUp` / `tearDown` with `init`,
- upgrades shared infrastructure (2 helper files) to support Swift Testing natively,
- introduces Swift Testing tags with Xcode Test Plans (`.xctestplan`) as the Xcode-native gate mechanism.

---

## 2. Product question this proposal must answer

After Proposal 009, the engineer must be able to answer all of these:

1. Can every non-UI executable unit test file (17 files) be authored and maintained using Swift Testing exclusively?
2. Does the shared test infrastructure (2 helper files: `TestSupport.swift`, `SharedMocks.swift`) work identically with both frameworks during the transition?
3. Are repetitive test methods consolidated into parameterized tests without losing individual test-case visibility in Xcode Test Navigator?
4. Can CI gates (`test-gate.sh`) select tests by Xcode Test Plans with tag filters instead of hard-coded `-only-testing:` class lists?
5. Do observation-heavy transport tests preserve request/session/close visibility after mock migration?

### Definition of done

Proposal 009 is done only when all of the following are true at once:

1. all 17 executable XCTest unit test files are migrated to Swift Testing;
2. both helper files (`TestSupport.swift`, `SharedMocks.swift`) provide Swift Testing-compatible APIs;
3. at least 3 test files use parameterized tests to consolidate previously duplicated methods;
4. at least one `.xctestplan` file exists and `test-gate.sh` can use it for tag-filtered gate execution;
5. no test regression: all tests that passed before continue to pass;
6. `Chainworks_ForgeUITests.swift` remains on XCTest unchanged.

---

## 3. What we change

Three scoped layers.

### Layer M: Test Infrastructure Upgrade

| Component | Responsibility |
|---|---|
| **TestSupport.swift** | Add `#expect`-based assertion helpers (`expectRunCompleted`, `expectRunBlocked`, `expectArtifactExists`, `expectArtifactNonEmpty`); add `confirmation()`-based async polling; retain XCTest variants until full migration completes; replace `try XCTUnwrap(...)` with `try #require(...)` in fixture loaders |
| **SharedMocks.swift** | Introduce two mock lanes (see section 6.3): lightweight `StubGooseTransport` for pure stream/result tests, and `ObservableGooseTransport` actor for tests that assert on request/session/close side effects; deprecate `@unchecked Sendable` class mocks |
| **Tag definitions** | Define `Tag` extensions for CI gate categories: `.fast`, `.smoke`, `.integration`, `.live`, `.provider` |
| **Xcode Test Plans** | Create `.xctestplan` files for `fast`, `provider`, and `full` gates with tag-based test selection; update `test-gate.sh` to use `-testPlan` flag |
| **TestBundleMarker** | Retain `NSObject`-based marker for `Bundle(for:)` fixture loading (Swift Testing has no bundle-discovery equivalent) |

### Layer N: File-by-File Migration

| Component | Responsibility |
|---|---|
| **17 executable XCTest unit test files** | Replace `import XCTest` → `import Testing`; convert `XCTestCase` class → `@Suite` struct; convert `func test*()` → `@Test func`; convert all `XCTAssert*` → `#expect` / `#require`; convert `setUp` → `init`; remove `tearDown` where struct lifecycle eliminates the need |

### Layer O: Parameterization and Tag Adoption

| Component | Responsibility |
|---|---|
| **TransitionEvaluatorTests** | Consolidate 25+ individual expression tests into parameterized tests |
| **GooseStreamEventMapperTests** | Consolidate 5 ignored-event tests and 3+ mapping tests into parameterized tests |
| **RunPlanCompilerTests** | Extract shared fixture loading into `init`; parameterize validation tests |
| **ArtifactValidationTests** | Parameterize contract validation test cases |
| **CI gate tags** | Apply `.fast`, `.smoke`, `.integration`, `.live` tags to suites matching `test-gate.sh` categories |

---

## 4. Migration contract

### 4.1 Framework replacement rules

Every migrated file must follow these rules:

| XCTest construct | Swift Testing replacement | Notes |
|---|---|---|
| `import XCTest` | `import Testing` | One framework per file; no mixing |
| `final class FooTests: XCTestCase` | `@Suite("Foo") struct FooTests` | Prefer structs; use `actor` only if shared mutable state is unavoidable |
| `func testSomething()` | `@Test func something()` | Drop `test` prefix; add display name via `@Test("description")` |
| `func testSomething() async throws` | `@Test func something() async throws` | Async support is identical |
| `override func setUp() async throws` | `init() throws` or `init() async throws` | Struct init runs per-test; no shared state leaks |
| `override func tearDown() async throws` | Remove | Struct deallocation handles cleanup; use `defer` in `init` for temp directories if needed |
| `XCTAssertTrue(x)` | `#expect(x)` | |
| `XCTAssertFalse(x)` | `#expect(!x)` | |
| `XCTAssertEqual(x, y)` | `#expect(x == y)` | |
| `XCTAssertEqual(x, y, "msg")` | `#expect(x == y, "msg")` | |
| `XCTAssertNotEqual(x, y)` | `#expect(x != y)` | |
| `XCTAssertNil(x)` | `#expect(x == nil)` | |
| `XCTAssertNotNil(x)` | `#expect(x != nil)` | |
| `XCTAssertGreaterThan(x, y)` | `#expect(x > y)` | |
| `XCTAssertGreaterThanOrEqual(x, y)` | `#expect(x >= y)` | |
| `XCTAssertLessThan(x, y)` | `#expect(x < y)` | |
| `try XCTUnwrap(x)` | `try #require(x)` | |
| `XCTAssertThrowsError(try f())` | `#expect(throws: (any Error).self) { try f() }` | |
| `XCTAssertThrowsError(try f()) { error in ... }` | `let e = #expect(throws: SomeError.self) { try f() }` | Returned error is typed |
| `XCTAssertNoThrow(try f())` | `#expect(throws: Never.self) { try f() }` | |
| `XCTFail("msg")` | `Issue.record("msg")` | |
| `XCTAssertIdentical(x, y)` | `#expect(x === y)` | |
| `continueAfterFailure = false` | Default behavior | Swift Testing stops test on `#require` failure |

### 4.2 Concurrency rules

Swift Testing does **not** run test functions on the main actor by default (unlike XCTest synchronous methods).

Rules:

- any test that accesses `@MainActor`-isolated code must either:
  - be in a `@MainActor` suite, or
  - use `@MainActor` on the individual `@Test` function, or
  - use `await MainActor.run { }` within the test body;
- tests that were previously synchronous XCTest methods running on the main actor by default must be explicitly annotated if they access main-actor-isolated state;
- tests that are already `async` and explicitly `@MainActor` require no change.

### 4.3 Serialization rules

Swift Testing runs tests in parallel by default within a suite.

Rules:

- suites that previously relied on XCTest's serial execution within a class must add `.serialized` trait;
- suites with independent tests (no shared mutable state) should use the default parallel behavior;
- `SharedEventCollector` and similar shared-state test doubles must remain `Sendable` regardless of suite serialization.

### 4.4 What must not migrate

| Component | Stays on XCTest | Reason |
|---|---|---|
| `Chainworks_ForgeUITests.swift` | Yes | Swift Testing has no `XCUIApplication` or UI testing equivalent |
| Any future `measure {}` performance tests | Yes | Swift Testing has no performance measurement API |
| `TestBundleMarker` (NSObject subclass) | Yes | `Bundle(for:)` requires an `NSObject` subclass; Swift Testing provides no alternative bundle discovery |

---

## 5. Parameterized test consolidation

### 5.1 TransitionEvaluatorTests — primary candidate

Current state: 25 individual test methods, many differing only in input values.

Target consolidation:

| Current methods | Parameterized replacement | Reduction |
|---|---|---|
| `testAlwaysReturnsTrue` | Single `@Test` for `.always` condition | 1 → 1 |
| `testArtifactExistsWhenPresent`, `testArtifactExistsWhenMissing` | `@Test(arguments:)` with `(artifacts, name, expected)` tuples | 2 → 1 |
| `testApprovalGrantedTrue`, `testApprovalGrantedFalse` | `@Test(arguments: [true, false])` | 2 → 1 |
| 6 expression literal/exists/approval tests | `@Test(arguments:)` with `(expression, context, expected)` tuples | 6 → 1 |
| 4 vars comparison tests | `@Test(arguments:)` with `(expression, variables, expected)` tuples | 4 → 1 |
| 2 artifact field tests | `@Test(arguments:)` with field data tuples | 2 → 1 |
| 4 and/or tests | `@Test(arguments:)` with compound expression tuples | 4 → 1 |
| 2 evaluateFirst tests | Remain individual (different assertion structure) | 2 → 2 |
| 3 edge case tests | `@Test(arguments:)` | 3 → 1 |

**Estimated reduction: 25 methods → 10 parameterized tests.** Each argument case still appears as an individual result in Xcode Test Navigator.

Example:

```swift
@Suite("TransitionEvaluator")
struct TransitionEvaluatorTests {

    private func makeContext(
        artifacts: Set<String> = [],
        approvalGranted: Bool = false,
        variables: [String: AnyCodableValue] = [:],
        artifactFields: [String: [String: AnyCodableValue]] = [:]
    ) -> TransitionEvaluator.EvaluationContext {
        TransitionEvaluator.EvaluationContext(
            producedArtifactNames: artifacts,
            approvalGranted: approvalGranted,
            variables: variables,
            artifactFields: artifactFields
        )
    }

    @Test("approval granted", arguments: [true, false])
    func approvalGranted(_ granted: Bool) {
        let ctx = makeContext(approvalGranted: granted)
        #expect(TransitionEvaluator.evaluate(.approvalGranted, context: ctx) == granted)
    }
}
```

### 5.2 GooseStreamEventMapperTests — ignored events

Current state: 5 identical test methods for ignored event types.

```swift
@Test("ignored events return nil", arguments: [
    #"{"type":"Ping"}"#,
    #"{"type":"Notification","request_id":"req_1","message":{}}"#,
    #"{"type":"UpdateConversation","conversation":{"messages":[]}}"#,
    #"{"type":"ActiveRequests","request_ids":["req_1","req_2"]}"#,
    #"{"type":"ModelChange","model":"default","mode":"lead"}"#,
])
func ignoredEventsReturnNil(_ json: String) {
    #expect(GooseStreamEventMapper.map(json) == nil)
}
```

**Reduction: 5 methods → 1 parameterized test.**

### 5.3 RunPlanCompilerTests — shared fixture loading

Current state: 10 of 15 methods begin with identical fixture-loading preamble.

After migration: `init()` loads fixtures once per test invocation; individual tests access `plan` directly.

```swift
@Suite("RunPlanCompiler")
@MainActor
struct RunPlanCompilerTests {
    let compiler: RunPlanCompiler
    let plan: RunPlan
    let container: ModelContainer
    let context: ModelContext

    init() throws {
        let (container, context) = try makeTestModelContainer()
        self.container = container
        self.context = context
        self.compiler = RunPlanCompiler(modelContext: context)
        let workflow = try loadTestCanonicalWorkflow()
        let catalog = try loadTestCanonicalCatalog()
        self.plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)
    }
}
```

**Reduction: ~40 lines of repeated setup eliminated.**

---

## 6. Shared infrastructure upgrade

### 6.1 TestSupport.swift — dual-mode assertion helpers

Add Swift Testing variants of every custom assertion helper:

```swift
import Testing

// MARK: - Swift Testing Assertion Helpers

func expectRunCompleted(_ run: Run, sourceLocation: SourceLocation = #_sourceLocation) {
    #expect(
        run.status == .completed,
        "Expected .completed, got .\(run.status.rawValue). "
        + "Stages: \(run.stageExecutions.map { "\($0.stageID)=\($0.status.rawValue)" }.joined(separator: ", "))",
        sourceLocation: sourceLocation
    )
}

func expectRunBlocked(_ run: Run, sourceLocation: SourceLocation = #_sourceLocation) {
    #expect(
        run.status == .blocked,
        "Expected .blocked, got .\(run.status.rawValue)",
        sourceLocation: sourceLocation
    )
}

func expectRunWaitingApproval(_ run: Run, sourceLocation: SourceLocation = #_sourceLocation) {
    #expect(
        run.status == .waitingApproval,
        "Expected .waitingApproval, got .\(run.status.rawValue)",
        sourceLocation: sourceLocation
    )
}

func expectArtifactExists(
    _ name: String,
    in run: Run,
    sourceLocation: SourceLocation = #_sourceLocation
) {
    let all = run.stageExecutions.flatMap(\.agentExecutions).flatMap(\.artifacts)
    #expect(
        all.contains { $0.name == name },
        "Artifact '\(name)' not found. Available: \(all.map(\.name).joined(separator: ", "))",
        sourceLocation: sourceLocation
    )
}

func expectArtifactNonEmpty(
    _ name: String,
    in run: Run,
    workspace: RunWorkspace,
    sourceLocation: SourceLocation = #_sourceLocation
) {
    let all = run.stageExecutions.flatMap(\.agentExecutions).flatMap(\.artifacts)
    guard let artifact = all.first(where: { $0.name == name }) else {
        Issue.record("Artifact '\(name)' not found in run", sourceLocation: sourceLocation)
        return
    }
    #expect(
        FileManager.default.fileExists(atPath: artifact.filePath),
        "Artifact '\(name)' file missing: \(artifact.filePath)",
        sourceLocation: sourceLocation
    )
    #expect(
        (artifact.sizeBytes ?? 0) > 0,
        "Artifact '\(name)' is empty (0 bytes)",
        sourceLocation: sourceLocation
    )
}
```

### 6.2 TestSupport.swift — confirmation-based async polling

Replace `pollUntil()` with a Swift Testing `confirmation()` variant:

```swift
@MainActor
func awaitCondition(
    _ description: String = "condition met",
    timeout: TimeInterval = 3.0,
    interval: TimeInterval = 0.05,
    condition: @escaping @Sendable () -> Bool
) async {
    await confirmation(description) { confirm in
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            if Date() > deadline { return }
            try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
        }
        confirm()
    }
}
```

### 6.3 SharedMocks.swift — two-lane mock strategy

The current `SharedMockGooseTransport` is a `final class ... @unchecked Sendable` that serves two distinct roles:

1. **Stimulus injection**: pre-configuring `createSessionResult`, `createSessionError`, and `streamEvents` to drive the code under test.
2. **Observation**: recording `closeSessionCalled`, `lastSessionRequest`, `createSessionCallCount`, and `submitPromptCallCount` so tests can assert on transport-side effects after execution.

Current observation-heavy tests (`GooseAgentExecutorTests`, `GooseSessionBridgeTests`, `OrchestratorTests`) assert on:

- `closeSessionCalled` — whether the session was properly closed
- `lastSessionRequest?.executionPolicy?.permissionProfileID` — policy propagation
- `lastSessionRequest?.executionPolicy?.workspaceMode` — workspace mode forwarding
- `lastSessionRequest?.executionPolicy?.gitOperationsAllowed` — git permission forwarding
- `lastSessionRequest?.executionPolicy?.releaseOperationsAllowed` — release permission forwarding
- `lastSessionRequest?.executionPolicy?.repoWritesAllowed` — repo write permission forwarding
- `createSessionCallCount` / `submitPromptCallCount` — call counting

A minimal struct stub cannot replace these observation requirements without losing test signal. Therefore this proposal introduces **two explicit mock lanes**:

#### Lane A: `StubGooseTransport` — lightweight value witness

For tests that only need stimulus injection (pre-configured responses and event streams) and do **not** assert on transport-side effects:

```swift
struct StubGooseTransport: GooseTransportProtocol, Sendable {
    var onCreateSession: @Sendable (GooseSessionRequest) async throws -> GooseSessionResponse = { _ in
        GooseSessionResponse(
            sessionId: "stub-\(UUID().uuidString.prefix(8))",
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true, capabilityToken: "stub", backendPolicyVersion: "v1"
            )
        )
    }
    var events: [GooseStreamEvent] = []

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        try await onCreateSession(request)
    }

    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        let events = self.events
        return AsyncThrowingStream { c in
            Task { for e in events { c.yield(e) }; c.finish() }
        }
    }

    func closeSession(sessionID: String) async throws {}
}
```

**Applicable to**: `GooseStreamEventMapperTests`, `SimulatedAgentExecutorTests`, stream-only tests in `GooseServerTransportTests`, and any new test that does not need observation.

#### Lane B: `ObservableGooseTransport` — actor-backed observable mock

For tests that need to assert on request content, session lifecycle, and call counts after execution:

```swift
actor ObservableGooseTransport: GooseTransportProtocol {
    // Stimulus configuration
    var createSessionResult: GooseSessionResponse?
    var createSessionError: Error?
    var streamEvents: [GooseStreamEvent] = []

    // Observable state
    private(set) var closeSessionCalled = false
    private(set) var lastSessionID: String?
    private(set) var lastSessionRequest: GooseSessionRequest?
    private(set) var createSessionCallCount = 0
    private(set) var submitPromptCallCount = 0

    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        createSessionCallCount += 1
        lastSessionRequest = request
        if let error = createSessionError { throw error }
        return createSessionResult ?? GooseSessionResponse(
            sessionId: "obs-\(UUID().uuidString.prefix(8))",
            status: "active",
            policyAcknowledgement: GoosePolicyAcknowledgement(
                accepted: true, capabilityToken: "obs", backendPolicyVersion: "v1"
            )
        )
    }

    func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        submitPromptCallCount += 1
        lastSessionID = sessionID
        let events = streamEvents
        return AsyncThrowingStream { c in
            Task { for e in events { c.yield(e) }; c.finish() }
        }
    }

    func closeSession(sessionID: String) async throws {
        closeSessionCalled = true
    }

    func reset() {
        closeSessionCalled = false
        lastSessionID = nil
        lastSessionRequest = nil
        createSessionCallCount = 0
        submitPromptCallCount = 0
    }
}
```

**Applicable to**: `GooseAgentExecutorTests` (asserts on `closeSessionCalled`, `lastSessionRequest.executionPolicy.*`), `GooseSessionBridgeTests` (asserts on session lifecycle), `OrchestratorTests` (asserts on call counts and request propagation).

**Key difference from current `SharedMockGooseTransport`**: `actor` provides compiler-verified `Sendable` safety without `@unchecked`. Observable state is accessed via `await` from tests, which is natural in `async` test functions.

#### Lane assignment per test file

| Test file | Current mock | Target lane | Reason |
|---|---|---|---|
| `GooseStreamEventMapperTests` | None (direct API) | N/A | No transport mock needed |
| `SimulatedAgentExecutorTests` | `SharedStaticResultExecutor` | Lane A (`StubGooseTransport`) if transport added | Stateless result injection |
| `GooseServerTransportTests` | Local class mock | Lane A for stream tests, Lane B for session tests | Mixed: some tests check streams only, others check session state |
| `GooseAgentExecutorTests` | Local `MockGooseTransport` class | **Lane B** (`ObservableGooseTransport`) | Asserts on `closeSessionCalled`, `lastSessionRequest.executionPolicy.*` |
| `GooseSessionBridgeTests` | `SharedMockGooseTransport` | **Lane B** (`ObservableGooseTransport`) | Asserts on session lifecycle |
| `OrchestratorTests` | `SharedMockGooseTransport` | **Lane B** (`ObservableGooseTransport`) | Asserts on call counts and request propagation |
| `EndToEndTests` | `SharedStaticResultExecutor` | Lane A | Stateless result injection |
| All other test files | Various | Lane A or N/A | No transport observation needed |

#### Transition rules

- retain `SharedMockGooseTransport` (class-based) during transition for files not yet migrated;
- new tests must use `StubGooseTransport` or `ObservableGooseTransport` depending on observation needs;
- `GooseAgentExecutorTests`'s local `MockGooseTransport` class is deleted when that file migrates to Lane B;
- remove `SharedMockGooseTransport` only after all dependents (`OrchestratorTests`, `GooseSessionBridgeTests`) are migrated to `ObservableGooseTransport`.

### 6.4 Tag definitions

```swift
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
```

### 6.5 Xcode Test Plans — the gate execution mechanism

#### Problem

The project currently has no `Package.swift` and no SwiftPM test invocation path.
`swift test --filter .tags(...)` is **not available** for this project.
All CI gates run through `xcodebuild` with `-only-testing:` arrays in `test-gate.sh`.

#### Solution

Xcode 16+ supports Swift Testing tags in `.xctestplan` files as test selection criteria.
This proposal introduces `.xctestplan` files as the Xcode-native bridge between Swift Testing tags and `xcodebuild` gate execution.

#### New project artifacts

```text
Chainworks Forge/
  TestPlans/
    FastGate.xctestplan          ← NEW
    ProviderGate.xctestplan      ← NEW
    FullGate.xctestplan          ← NEW
```

Each `.xctestplan` file is a JSON document that Xcode manages through the Test Plan editor. The plan specifies:

- which test target(s) to include,
- which tags to include/exclude,
- environment variables and launch arguments.

Example structure for `FastGate.xctestplan` (human-readable summary; actual file is Xcode-managed JSON):

- **Included targets**: `Chainworks ForgeTests`
- **Tag filter**: include tests tagged `.fast`
- **Excluded targets**: `Chainworks ForgeUITests` (UI smoke has its own plan/gate)

#### Updated `test-gate.sh` invocations

```bash
# Current (hard-coded class list):
FAST_TESTS=(
  "Chainworks ForgeTests/ProviderPlatformTests"
  "Chainworks ForgeTests/OrchestratorTests"
  "Chainworks ForgeTests/ResumeManagerTests"
  "Chainworks ForgeTests/ArtifactManagerTests"
  "Chainworks ForgeTests/RunTests"
)
xcodebuild test -only-testing:"${FAST_TESTS[@]}" ...

# After (test plan with tag filter):
xcodebuild test \
  -project "$PROJECT_PATH" \
  -scheme "$SCHEME_NAME" \
  -destination "$DESTINATION" \
  -testPlan FastGate \
  -derivedDataPath "$derived_data" \
  -resultBundlePath "$result_bundle"
```

#### Tag-to-gate mapping

| CI gate | Current mechanism | New mechanism | Test plan |
|---|---|---|---|
| `fast` | Hard-coded `-only-testing:` array of 5 class names | `-testPlan FastGate` with tag `.fast` | `FastGate.xctestplan` |
| `ui-smoke` | Hard-coded `-only-testing:` array of 5 method names | **No change** — remains `-only-testing:` (XCUITest, not tag-eligible) | N/A |
| `proposal-006` | Hard-coded `-only-testing:` mixed unit + UI | `-testPlan ProviderGate` for unit tests (tag `.provider`); UI tests remain `-only-testing:` | `ProviderGate.xctestplan` |
| `full` | `xcodebuild test` (all tests) | `-testPlan FullGate` or no change | `FullGate.xctestplan` (optional) |

#### Backward compatibility

During migration, `test-gate.sh` must support both paths:

1. If the `.xctestplan` file exists and the `USE_TEST_PLANS=1` environment variable is set, use `-testPlan`.
2. Otherwise, fall back to the current `-only-testing:` arrays.

This allows incremental adoption and prevents CI breakage during the transition.

#### What stays on `-only-testing:`

- **UI smoke gate** (`ui-smoke`): XCUITest methods are not eligible for Swift Testing tags. This gate continues to use hard-coded `-only-testing:` method references.
- **proposal-006 UI portion**: the 3 UI test methods in `PROPOSAL_006_TESTS` remain `-only-testing:`-selected.

---

## 7. File-by-file migration plan

### 7.1 Priority tiers

**Migration scope: 17 executable XCTest unit test files.**
Helper files (`TestSupport.swift`, `SharedMocks.swift`) are upgraded in Tier 1 but are not counted as migration targets because they contain no executable tests.

| Priority | File | Lines | Complexity | Key change | Mock lane |
|---|---|---|---|---|---|
| **Tier 1: Infrastructure (2 helper files + 2 new files)** | | | | | |
| 1a | `TestSupport.swift` | 314 | Medium | Add `#expect` helpers; retain XCTest variants | — |
| 1b | `SharedMocks.swift` | 128 | Medium | Add `StubGooseTransport` (Lane A) + `ObservableGooseTransport` (Lane B); retain class mocks | — |
| 1c | `TestTags.swift` (new) | ~20 | Low | Define `Tag` extensions | — |
| 1d | `TestPlans/` (new) | — | Low | Create `FastGate.xctestplan`, `ProviderGate.xctestplan`, `FullGate.xctestplan` | — |
| **Tier 2: Pure logic — no SwiftData, no async (2 files)** | | | | | |
| 2a | `TransitionEvaluatorTests.swift` | 218 | Low | Parameterize 25 → 10 tests | N/A |
| 2b | `GooseStreamEventMapperTests.swift` | 195 | Low | Parameterize ignored + mapping tests | N/A |
| **Tier 3: SwiftData + sync (3 files)** | | | | | |
| 3a | `RunPlanCompilerTests.swift` | 289 | Medium | `init()` replaces setUp; shared fixture loading | N/A |
| 3b | `ArtifactValidationTests.swift` | 264 | Medium | Parameterize contract validations | N/A |
| 3c | `ArtifactManagerTests.swift` | 347 | Medium | `init()` replaces setUp/tearDown; temp dir via UUID | N/A |
| **Tier 4: SwiftData + async (3 files)** | | | | | |
| 4a | `SimulatedAgentExecutorTests.swift` | 228 | Medium | Struct suite with `@MainActor` | Lane A |
| 4b | `ResumeManagerTests.swift` | 418 | Medium | `.serialized` trait; tag `.fast` | N/A |
| 4c | `WorkspaceIsolationTests.swift` | 336 | Medium | `.serialized` trait | N/A |
| **Tier 5: Complex async + mocks (5 files)** | | | | | |
| 5a | `GooseAgentExecutorTests.swift` | 371 | High | Delete local `MockGooseTransport` class; use `ObservableGooseTransport` (Lane B); assert on `closeSessionCalled`, `lastSessionRequest.executionPolicy.*` via `await` | **Lane B** |
| 5b | `GooseSessionBridgeTests.swift` | 307 | High | `confirmation()` replaces polling; `ObservableGooseTransport` (Lane B) | **Lane B** |
| 5c | `GooseServerTransportTests.swift` | 652 | High | Stream-only tests use Lane A; session lifecycle tests use Lane B | **Lane A + B** |
| 5d | `ProviderPlatformTests.swift` | 719 | High | Tag `.fast` + `.provider` | N/A |
| 5e | `OrchestratorTests.swift` | 1077 | High | Largest file; `.serialized`; tag `.fast`; `ObservableGooseTransport` (Lane B) | **Lane B** |
| **Tier 6: Integration / live (4 files)** | | | | | |
| 6a | `EndToEndTests.swift` | 404 | High | Tag `.integration` | Lane A |
| 6b | `LiveGooseConnectionProofTests.swift` | 595 | High | Tag `.live`; `.disabled` trait | N/A |
| 6c | `GooseServerLiveIntegrationTests.swift` | 185 | High | Tag `.live`; `.timeLimit(.minutes(2))` | N/A |
| 6d | `LiveProposalWorkflowTests.swift` | 230 | High | Tag `.live`; `.disabled` trait | N/A |
| **Not migrated** | | | | | |
| — | `Chainworks_ForgeTests.swift` | 1086 | — | Already on Swift Testing | — |
| — | `Chainworks_ForgeUITests.swift` | — | — | Remains XCTest (UI tests) | — |

**Verification**: Tiers 2–6 contain exactly **17 files** (2 + 3 + 3 + 5 + 4), matching the executable XCTest unit test inventory from section 1.

### 7.2 Migration sequence constraint

Tier 1 (infrastructure) must complete before any Tier 2+ file is migrated.
Within Tiers 2-6, files are independent and can be migrated in any order.

---

## 8. Concrete migration examples

### 8.1 ArtifactManagerTests — setUp/tearDown elimination

**Before:**

```swift
import XCTest
import SwiftData
@testable import Chainworks_Forge

@MainActor
final class ArtifactManagerTests: XCTestCase {
    var container: ModelContainer!
    var context: ModelContext!
    var manager: ArtifactManager!
    var tempDir: URL!

    override func setUp() async throws {
        let schema = Schema([Idea.self, Run.self, StageExecution.self,
                             AgentExecution.self, Approval.self, Artifact.self])
        let config = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
        container = try ModelContainer(for: schema, configurations: [config])
        context = container.mainContext
        manager = ArtifactManager(modelContext: context)
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ArtifactManagerTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let dir = tempDir, FileManager.default.fileExists(atPath: dir.path) {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    func testPersistOutputsWritesToDiskAndSwiftData() throws {
        // ... test body using container, context, manager, tempDir ...
    }
}
```

**After:**

```swift
import Testing
import SwiftData
@testable import Chainworks_Forge

@Suite("ArtifactManager", .tags(.fast))
@MainActor
struct ArtifactManagerTests {
    let container: ModelContainer
    let context: ModelContext
    let manager: ArtifactManager
    let tempDir: URL

    init() throws {
        let (container, context) = try makeTestModelContainer()
        self.container = container
        self.context = context
        self.manager = ArtifactManager(modelContext: context)
        self.tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ArtifactManagerTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        // No tearDown needed: UUID-based temp dir avoids conflicts;
        // OS cleans /tmp; struct value semantics prevent state leaks.
    }

    @Test("persist outputs writes to disk and SwiftData")
    func persistOutputs() throws {
        // ... test body using container, context, manager, tempDir ...
    }
}
```

### 8.2 RunPlanCompilerTests — transition condition parsing

**Before:**

```swift
func testTransitionConditionParsing() throws {
    let workflow = try loadCanonicalWorkflow()
    let catalog = try loadCanonicalCatalog()
    let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

    let state1 = plan.states["state_1_idea_received"]!
    if case .artifactExists(let name) = state1.transitions.first?.condition {
        XCTAssertEqual(name, "idea_brief")
    } else {
        XCTFail("Expected .artifactExists condition for state_1")
    }

    let state3 = plan.states["state_3_initial_proposal_approval"]!
    if case .approvalGranted = state3.transitions.first?.condition {
        // pass
    } else {
        XCTFail("Expected .approvalGranted condition for state_3")
    }
}
```

**After:**

```swift
@Test("transition condition parsing — state_1 uses artifactExists")
func transitionConditionState1() throws {
    let state1 = try #require(plan.states["state_1_idea_received"])
    let condition = try #require(state1.transitions.first?.condition)
    guard case .artifactExists(let name) = condition else {
        Issue.record("Expected .artifactExists, got \(condition)")
        return
    }
    #expect(name == "idea_brief")
}

@Test("transition condition parsing — state_3 uses approvalGranted")
func transitionConditionState3() throws {
    let state3 = try #require(plan.states["state_3_initial_proposal_approval"])
    let condition = try #require(state3.transitions.first?.condition)
    guard case .approvalGranted = condition else {
        Issue.record("Expected .approvalGranted, got \(condition)")
        return
    }
}
```

### 8.3 GooseAgentExecutorTests — observable mock migration (Lane B)

**Before** (class-based `@unchecked Sendable` mock with direct property access):

```swift
import XCTest
@testable import Chainworks_Forge

final class GooseAgentExecutorTests: XCTestCase {
    final class MockGooseTransport: GooseTransportProtocol, @unchecked Sendable {
        var closeSessionCalled = false
        var lastSessionRequest: GooseSessionRequest?
        // ...
    }

    @MainActor
    func testGooseExecutorCreatesSession() async throws {
        let mockTransport = MockGooseTransport()
        // ... configure and execute ...
        XCTAssertTrue(mockTransport.closeSessionCalled)
        XCTAssertEqual(mockTransport.lastSessionRequest?.executionPolicy?.permissionProfileID, "read_only")
        XCTAssertEqual(mockTransport.lastSessionRequest?.executionPolicy?.workspaceMode, "read_only")
        XCTAssertEqual(mockTransport.lastSessionRequest?.executionPolicy?.gitOperationsAllowed, false)
    }
}
```

**After** (actor-based `Sendable` observable mock with `await` access):

```swift
import Testing
@testable import Chainworks_Forge

@Suite("GooseAgentExecutor")
@MainActor
struct GooseAgentExecutorTests {
    @Test("creates session with correct execution policy")
    func createsSession() async throws {
        let transport = ObservableGooseTransport()
        await transport.configure(
            sessionResult: GooseSessionResponse(
                sessionId: "session-abc123", status: "active",
                policyAcknowledgement: GoosePolicyAcknowledgement(
                    accepted: true, capabilityToken: "mock-read-only",
                    backendPolicyVersion: "mock-v1"
                )
            ),
            events: [
                .sessionStarted(raw: "{}"),
                .finalOutput(content: "# Test Output"),
                .sessionClosed(raw: "{}")
            ]
        )

        let executor = GooseAgentExecutor(transport: transport)
        _ = try await executor.execute(task: makeTask(), agent: makeAgent(), context: makeContext())

        // Observable state accessed via await — compiler-verified Sendable safety
        #expect(await transport.closeSessionCalled, "Session should be closed after execution")
        let request = await transport.lastSessionRequest
        #expect(request?.executionPolicy?.permissionProfileID == "read_only")
        #expect(request?.executionPolicy?.workspaceMode == "read_only")
        #expect(request?.executionPolicy?.gitOperationsAllowed == false)
    }
}
```

**Key changes**: `XCTAssertTrue(mock.prop)` → `#expect(await transport.prop)`. The `await` is natural in `async` test functions and provides compiler-verified thread safety without `@unchecked Sendable`.

### 8.4 Live tests — traits for conditional execution

**Before:**

```swift
final class LiveGooseConnectionProofTests: XCTestCase {
    func testLiveServerConnection() async throws {
        // Requires running Goose server
        // Often skipped manually or fails in CI
    }
}
```

**After:**

```swift
@Suite("Live Goose Connection", .tags(.live), .timeLimit(.minutes(2)))
struct LiveGooseConnectionProofTests {
    @Test(.disabled("Requires running Goose server; enable for manual validation"))
    func liveServerConnection() async throws {
        // ...
    }
}
```

---

## 9. Acceptance criteria

### Infrastructure

- [ ] `TestSupport.swift` provides `expectRunCompleted`, `expectRunBlocked`, `expectRunWaitingApproval`, `expectArtifactExists`, `expectArtifactNonEmpty` using `#expect`
- [ ] `TestSupport.swift` provides `awaitCondition()` using `confirmation()`
- [ ] `TestSupport.swift` replaces `try XCTUnwrap(...)` in fixture loaders with `try #require(...)`
- [ ] `SharedMocks.swift` provides `StubGooseTransport` (Lane A) as a `Sendable` struct
- [ ] `SharedMocks.swift` provides `ObservableGooseTransport` (Lane B) as a `Sendable` actor with request/session/close observability
- [ ] Tag extensions define `.fast`, `.smoke`, `.integration`, `.live`, `.provider` in `TestTags.swift`
- [ ] `TestBundleMarker` remains functional for fixture loading

### Mock migration

- [ ] `GooseAgentExecutorTests` uses `ObservableGooseTransport` (Lane B) and asserts on `closeSessionCalled`, `lastSessionRequest.executionPolicy.*` via `await`
- [ ] `GooseSessionBridgeTests` uses `ObservableGooseTransport` (Lane B) for session lifecycle assertions
- [ ] `OrchestratorTests` uses `ObservableGooseTransport` (Lane B) for call count and request propagation assertions
- [ ] Local `MockGooseTransport` class in `GooseAgentExecutorTests` is deleted
- [ ] `SharedMockGooseTransport` class is removed only after all dependents are migrated

### Migration

- [ ] All 17 executable XCTest unit test files use `import Testing` instead of `import XCTest`
- [ ] All test classes are replaced with `@Suite` structs (or actors where required)
- [ ] All `XCTAssert*` calls are replaced with `#expect` / `#require`
- [ ] All `override func setUp()` are replaced with `init()`
- [ ] All `override func tearDown()` are removed or replaced with `defer` in `init`
- [ ] All `XCTFail` calls are replaced with `Issue.record`
- [ ] All `try XCTUnwrap` calls are replaced with `try #require`
- [ ] `@MainActor` is explicitly applied where needed (not relying on XCTest's implicit main-thread execution)

### Parameterization

- [ ] `TransitionEvaluatorTests` consolidates to ≤ 12 `@Test` functions (from 25)
- [ ] `GooseStreamEventMapperTests` consolidates ignored-event tests into 1 parameterized test
- [ ] At least one additional file uses parameterized tests

### Tags and CI

- [ ] Suites matching the `fast` CI gate are tagged `.fast`
- [ ] Live/integration test suites are tagged `.live` or `.integration`
- [ ] Provider-specific suites are tagged `.provider`
- [ ] `FastGate.xctestplan` exists and selects tests by `.fast` tag
- [ ] `ProviderGate.xctestplan` exists and selects tests by `.provider` tag
- [ ] `test-gate.sh` supports `-testPlan` invocation when `USE_TEST_PLANS=1` is set
- [ ] `test-gate.sh` retains backward-compatible `-only-testing:` fallback when `USE_TEST_PLANS` is not set
- [ ] UI smoke gate (`ui-smoke`) remains on `-only-testing:` (XCUITest, not tag-eligible)

### Regression

- [ ] All tests that passed before migration continue to pass
- [ ] No new test failures introduced by the migration
- [ ] `Chainworks_ForgeUITests.swift` is unchanged
- [ ] CI pipeline remains green on all gates

---

## 10. Out of scope

| Exclusion | Reason |
|---|---|
| UI test migration | Swift Testing has no `XCUIApplication` equivalent |
| Performance test migration | Swift Testing has no `measure {}` equivalent |
| New test coverage | This proposal modernizes framework usage, not coverage breadth |
| `test-gate.sh` full rewrite | Script receives optional `USE_TEST_PLANS` path but retains backward-compatible `-only-testing:` fallback |
| `Package.swift` / SwiftPM adoption | The project remains Xcode-project-only; `swift test` is not a goal of this proposal |
| Third-party testing frameworks (Quick, Nimble, etc.) | Swift Testing is Apple's first-party replacement |
| Swift 6 strict concurrency migration | Orthogonal to test framework migration; may be addressed separately |

---

## 11. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| TEST-001 | All new unit tests must use Swift Testing (`import Testing`) | Apple's current guidance; project already on Xcode 16+ |
| TEST-002 | XCTest remains exclusively for UI tests and performance tests | Swift Testing has no equivalent for `XCUIApplication` or `measure {}` |
| TEST-003 | Migrated suites must use structs, not classes | Value-type semantics provide better test isolation and concurrency safety |
| TEST-004 | `@unchecked Sendable` class mocks are replaced with either `Sendable` struct stubs (Lane A) or `Sendable` actor observables (Lane B) depending on observation requirements | Compiler-verified concurrency safety; observation-heavy tests retain request/session/close visibility |
| TEST-005 | Parameterized tests must be used when 3+ methods differ only in input data | Reduces maintenance burden and improves test readability |
| TEST-006 | CI gate categories are expressed as `Tag` extensions and selected via Xcode Test Plans (`.xctestplan`), not `swift test --filter` | The project has no `Package.swift`; `xcodebuild -testPlan` is the only Xcode-native tag-selection path |
| TEST-007 | `TestBundleMarker` (NSObject) is retained for fixture loading | No Swift Testing alternative exists for `Bundle(for:)` |
| TEST-008 | Migration is file-atomic: each file moves completely from XCTest to Swift Testing | No per-file mixing of `XCTAssert*` and `#expect` |
| TEST-009 | Infrastructure tier (TestSupport, SharedMocks, Tags) completes before any test file migration | Prevents dual-maintenance of assertion helpers |
| TEST-010 | XCTest assertion helpers in `TestSupport.swift` are removed only after all dependents are migrated | Ensures no broken imports during incremental migration |

---

## 12. Estimated impact

| Metric | Before | After | Change |
|---|---|---|---|
| Executable XCTest unit files | 17 | 0 | -17 |
| Executable Swift Testing unit files | 1 | 18 | +17 |
| Helper files upgraded | 0 | 2 (`TestSupport`, `SharedMocks`) | +2 (not counted as migration targets) |
| UI test files (XCTest, unchanged) | 1 | 1 | 0 |
| Total test methods | ~180 | ~130 | ~-28% (parameterization) |
| `XCTAssert*` call sites | ~350 | 0 | -100% |
| `override func setUp` | 7 | 0 | -100% |
| `override func tearDown` | 6 | 0 | -100% |
| Force-unwrapped test properties (`!`) | ~30 | 0 | -100% |
| `@unchecked Sendable` test doubles | 2+ (class-based) | 0 (struct Lane A + actor Lane B) | -100% |
| Xcode Test Plans | 0 | 3 (`FastGate`, `ProviderGate`, `FullGate`) | +3 |
| Lines of test code (estimated) | ~9,100 | ~7,500 | ~-18% |

---

## 13. Execution plan

| Day | Deliverable | Files touched |
|---|---|---|
| Day 1 | Tier 1: upgrade `TestSupport.swift` with dual-mode helpers; add `StubGooseTransport` (Lane A) + `ObservableGooseTransport` (Lane B) to `SharedMocks.swift`; create `TestTags.swift`; create `TestPlans/FastGate.xctestplan`, `ProviderGate.xctestplan`, `FullGate.xctestplan` | 2 helpers + 4 new files |
| Day 2 | Tier 2: migrate `TransitionEvaluatorTests` and `GooseStreamEventMapperTests` with full parameterization | 2 of 17 |
| Day 3 | Tier 3: migrate `RunPlanCompilerTests`, `ArtifactValidationTests`, `ArtifactManagerTests` | 5 of 17 |
| Day 4 | Tier 4: migrate `SimulatedAgentExecutorTests`, `ResumeManagerTests`, `WorkspaceIsolationTests` | 8 of 17 |
| Day 5 | Tier 5: migrate `GooseAgentExecutorTests` (Lane B), `GooseSessionBridgeTests` (Lane B), `GooseServerTransportTests` (Lane A+B), `ProviderPlatformTests`, `OrchestratorTests` (Lane B) | 13 of 17 |
| Day 6 | Tier 6: migrate `EndToEndTests`, `LiveGooseConnectionProofTests`, `GooseServerLiveIntegrationTests`, `LiveProposalWorkflowTests` | 17 of 17 |
| Day 7 | Remove XCTest assertion helpers from `TestSupport.swift`; remove `SharedMockGooseTransport` class and local `MockGooseTransport` classes from `SharedMocks.swift`; update `test-gate.sh` with `USE_TEST_PLANS` support; final CI verification on all gates | Cleanup + CI |

---

## 14. What this proposal enables

Proposal 009 converts the test suite from a framework that Apple recommends migrating away from to the framework Apple recommends migrating toward.

It enables:

- a test suite where all 18 unit test files use the same framework conventions as `Chainworks_ForgeTests.swift` (already migrated),
- parameterized tests that reduce maintenance burden by ~28% in method count and ~18% in line count,
- compiler-verified `Sendable` safety in all test doubles (struct stubs for stateless tests, actor observables for side-effect assertions),
- declarative CI gate configuration through Xcode Test Plans with tag filters instead of hard-coded `-only-testing:` class lists,
- a foundation for Swift 6 strict concurrency adoption in the test target,
- and a codebase where new tests are written using modern idioms from day one.
