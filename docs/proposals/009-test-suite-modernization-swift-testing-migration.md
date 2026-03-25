# Test Suite Architecture — Swift Testing Migration

Implemented reference for the Chainworks Forge test suite after migration from XCTest to Swift Testing.

| Field | Value |
|---|---|
| Status | Implemented (see [Known Gaps](#known-gaps) for residual items) |
| Scope | Unit test target (`Chainworks ForgeTests/`) |
| Framework | Swift Testing (Xcode 16+, Swift 5) |
| UI tests | Remain on XCTest — not migrated |

---

## Table of Contents

1. [Inventory](#1-inventory)
2. [Framework Conventions](#2-framework-conventions)
3. [Concurrency Rules](#3-concurrency-rules)
4. [Mock Strategy](#4-mock-strategy)
5. [Test Infrastructure](#5-test-infrastructure)
6. [Parameterized Tests](#6-parameterized-tests)
7. [Tags and Test Plans](#7-tags-and-test-plans)
8. [CI Gate Integration](#8-ci-gate-integration)
9. [What Stays on XCTest](#9-what-stays-on-xctest)
10. [Known Gaps](#10-known-gaps)

---

## 1. Inventory

The unit test target (`Chainworks ForgeTests/`) contains 20 Swift files:

| Category | Count | Notes |
|---|---|---|
| Executable Swift Testing unit tests | 18 | All use `import Testing`, `@Suite` structs |
| Shared helper files | 2 | `TestSupport.swift`, `SharedMocks.swift` — not executable tests |

The UI test target (`Chainworks ForgeUITests/`) contains 1 XCUITest file, unchanged.

### Executable test files

| File | Lines | Tags | Mock Lane |
|---|---|---|---|
| `ArtifactManagerTests` | 347 | `.fast` | — |
| `ArtifactValidationTests` | 264 | — | — |
| `Chainworks_ForgeTests` | 1086 | `.fast` | — |
| `EndToEndTests` | 404 | `.integration` | Lane A |
| `GooseAgentExecutorTests` | 371 | — | **Lane B** |
| `GooseServerLiveIntegrationTests` | 185 | `.live` | — |
| `GooseServerTransportTests` | 652 | — | Local mock (see [Known Gaps](#10-known-gaps)) |
| `GooseSessionBridgeTests` | 307 | — | **Lane B** |
| `GooseStreamEventMapperTests` | 195 | — | — |
| `LiveGooseConnectionProofTests` | 595 | `.live` | — |
| `LiveProposalWorkflowTests` | 230 | `.live` | — |
| `OrchestratorTests` | 1077 | `.fast` | **Lane B** |
| `ProviderPlatformTests` | 719 | `.fast`, `.provider` | — |
| `ResumeManagerTests` | 418 | `.fast` | — |
| `RunPlanCompilerTests` | 289 | — | — |
| `SimulatedAgentExecutorTests` | 228 | — | Lane A |
| `TransitionEvaluatorTests` | 218 | — | — |
| `WorkspaceIsolationTests` | 336 | — | — |

### Helper files

| File | Role |
|---|---|
| `TestSupport.swift` | Assertion helpers, fixture loaders, async polling, `TestBundleMarker` |
| `SharedMocks.swift` | `StubGooseTransport` (Lane A), `ObservableGooseTransport` (Lane B), `SharedStaticResultExecutor`, `SharedEventCollector` |

---

## 2. Framework Conventions

Every migrated unit test file follows these conventions:

| Pattern | Convention |
|---|---|
| Import | `import Testing` — one framework per file, no mixing |
| Suite declaration | `@Suite("Display Name") struct FooTests` — structs, not classes |
| Test function | `@Test("description") func something() async throws` — no `test` prefix |
| Setup | `init() throws` or `init() async throws` — runs per test invocation |
| Teardown | Removed — struct deallocation handles cleanup; `defer` in `init` for temp dirs |
| Assertions | `#expect(condition)` / `#expect(x == y)` |
| Unwrap | `try #require(x)` replaces `try XCTUnwrap(x)` |
| Failure | `Issue.record("msg")` replaces `XCTFail("msg")` |
| Throws | `#expect(throws: SomeError.self) { try f() }` |
| No-throw | `#expect(throws: Never.self) { try f() }` |
| Identity | `#expect(x === y)` |

### Assertion mapping reference

| XCTest | Swift Testing |
|---|---|
| `XCTAssertTrue(x)` | `#expect(x)` |
| `XCTAssertFalse(x)` | `#expect(!x)` |
| `XCTAssertEqual(x, y)` | `#expect(x == y)` |
| `XCTAssertNotEqual(x, y)` | `#expect(x != y)` |
| `XCTAssertNil(x)` | `#expect(x == nil)` |
| `XCTAssertNotNil(x)` | `#expect(x != nil)` |
| `XCTAssertGreaterThan(x, y)` | `#expect(x > y)` |
| `XCTAssertLessThan(x, y)` | `#expect(x < y)` |
| `try XCTUnwrap(x)` | `try #require(x)` |
| `XCTFail("msg")` | `Issue.record("msg")` |
| `XCTAssertIdentical(x, y)` | `#expect(x === y)` |

---

## 3. Concurrency Rules

Swift Testing does **not** run test functions on the main actor by default.

| Rule | Detail |
|---|---|
| `@MainActor` access | Suite must be `@MainActor`, or individual `@Test` must be `@MainActor`, or use `await MainActor.run {}` |
| Serialization | Suites with shared mutable state use `.serialized` trait; independent suites use default parallel |
| Sendable | `SharedEventCollector` and similar shared-state test doubles must remain `Sendable` regardless of suite serialization |

Suites using `.serialized`: `OrchestratorTests`, `ResumeManagerTests`, `WorkspaceIsolationTests`.

---

## 4. Mock Strategy

Two explicit mock lanes replace the previous `SharedMockGooseTransport` class:

### Lane A: `StubGooseTransport` — lightweight value witness

`Sendable` struct for tests that only need stimulus injection (pre-configured responses and event streams) and do **not** assert on transport-side effects.

```swift
struct StubGooseTransport: GooseTransportProtocol, Sendable {
    var onCreateSession: @Sendable (GooseSessionRequest) async throws -> GooseSessionResponse
    var events: [GooseStreamEvent] = []
    // ...
}
```

Used by: `SimulatedAgentExecutorTests`, `EndToEndTests`, stream-only paths.

### Lane B: `ObservableGooseTransport` — actor-backed observable mock

`actor` for tests that assert on request content, session lifecycle, and call counts:

```swift
actor ObservableGooseTransport: GooseTransportProtocol {
    // Observable state
    private(set) var closeSessionCalled: Bool
    private(set) var lastSessionRequest: GooseSessionRequest?
    private(set) var createSessionCallCount: Int
    private(set) var submitPromptCallCount: Int
    // ...
}
```

Observable state is accessed via `await` — compiler-verified `Sendable` safety without `@unchecked`.

Used by: `GooseAgentExecutorTests`, `GooseSessionBridgeTests`, `OrchestratorTests`.

### Lane assignment

| Test file | Lane | Reason |
|---|---|---|
| `GooseAgentExecutorTests` | **B** | Asserts on `closeSessionCalled`, `lastSessionRequest.executionPolicy.*` |
| `GooseSessionBridgeTests` | **B** | Asserts on session lifecycle |
| `OrchestratorTests` | **B** | Asserts on call counts and request propagation |
| `SimulatedAgentExecutorTests` | A | Stateless result injection |
| `EndToEndTests` | A | Stateless result injection |
| `GooseStreamEventMapperTests` | — | No transport mock needed |
| `GooseServerTransportTests` | Local mock | See [Known Gaps](#10-known-gaps) |

---

## 5. Test Infrastructure

### TestSupport.swift

#### Swift Testing assertion helpers

| Helper | Purpose |
|---|---|
| `expectRunCompleted(_:)` | Asserts `run.status == .completed` with diagnostic stage dump |
| `expectRunBlocked(_:)` | Asserts `run.status == .blocked` |
| `expectRunWaitingApproval(_:)` | Asserts `run.status == .waitingApproval` |
| `expectArtifactExists(_:in:)` | Asserts named artifact exists in run's artifact collection |
| `expectArtifactNonEmpty(_:in:workspace:)` | Asserts artifact file exists on disk and has nonzero size |

All helpers accept `sourceLocation: SourceLocation = #_sourceLocation` for accurate failure reporting.

#### Async polling

`awaitCondition(_:timeout:interval:condition:)` — polls a condition with configurable timeout and interval. Records an issue on timeout.

#### Fixture loading

`TestBundleMarker` (`NSObject` subclass) provides `Bundle(for:)` access for loading test fixtures. Swift Testing has no bundle-discovery equivalent, so this stays as-is.

Fixture loaders: `loadTestCanonicalWorkflow()`, `loadTestCanonicalCatalog()`, `makeTestModelContainer()`.

### SharedMocks.swift

| Component | Type | Role |
|---|---|---|
| `StubGooseTransport` | `struct` (Sendable) | Lane A — stimulus injection |
| `ObservableGooseTransport` | `actor` | Lane B — observable mock |
| `SharedStaticResultExecutor` | struct | Static result injection for E2E tests |
| `SharedEventCollector` | class (`@unchecked Sendable`) | Event collection (see [Known Gaps](#10-known-gaps)) |

### TestTags.swift

Tag extensions for CI gate categories:

```swift
extension Tag {
    @Tag static var fast: Self
    @Tag static var smoke: Self
    @Tag static var integration: Self
    @Tag static var live: Self
    @Tag static var provider: Self
}
```

---

## 6. Parameterized Tests

Three files use `@Test(arguments:)` to consolidate repetitive test methods:

### TransitionEvaluatorTests

25 individual test methods consolidated to ~10 parameterized tests. Each argument case still appears individually in Xcode Test Navigator.

Example:

```swift
@Test("approval granted", arguments: [true, false])
func approvalGranted(_ granted: Bool) {
    let ctx = makeContext(approvalGranted: granted)
    #expect(TransitionEvaluator.evaluate(.approvalGranted, context: ctx) == granted)
}
```

### GooseStreamEventMapperTests

5 identical ignored-event test methods consolidated to 1 parameterized test:

```swift
@Test("ignored events return nil", arguments: [
    #"{"type":"Ping"}"#,
    #"{"type":"Notification","request_id":"req_1","message":{}}"#,
    // ...
])
func ignoredEventsReturnNil(_ json: String) {
    #expect(GooseStreamEventMapper.map(json) == nil)
}
```

### ResumeManagerTests

Additional parameterized classification test for resume scenarios.

---

## 7. Tags and Test Plans

### Tags

Tags are the single source of truth for gate membership. Defined in `TestTags.swift`.

| Tag | Purpose | Suites |
|---|---|---|
| `.fast` | Inner-loop engineering gate | `Chainworks_ForgeTests`, `OrchestratorTests`, `ProviderPlatformTests`, `ResumeManagerTests`, `ArtifactManagerTests` |
| `.smoke` | UI-level smoke coverage (unit-side only) | — |
| `.integration` | External provider connectivity | `EndToEndTests` |
| `.live` | Running Goose server required | `GooseServerLiveIntegrationTests`, `LiveGooseConnectionProofTests`, `LiveProposalWorkflowTests` |
| `.provider` | Provider-specific (Proposal 006) | `ProviderPlatformTests` |

### Test Plans

Located in `TestPlans/`:

| Plan | Tag filter | Targets |
|---|---|---|
| `FastGate.xctestplan` | `.fast` suites | `Chainworks ForgeTests` |
| `ProviderGate.xctestplan` | `.provider` suites | `Chainworks ForgeTests` |
| `FullGate.xctestplan` | All | `Chainworks ForgeTests` + `Chainworks ForgeUITests` |

Plans use `selectedTests` lists bridged from `@Tag` declarations by the `guard_plan_tag_sync` guardrail in `test-gate.sh`.

> **Note on tag filtering:** Neither `xcodebuild` nor the `.xctestplan` format support runtime Swift Testing `Tag` filtering as of Xcode 16 / Swift 5. The implementation bridges this via `selectedTests` lists, kept in sync with tag declarations by `guard_plan_tag_sync`.

---

## 8. CI Gate Integration

`test-gate.sh` supports two execution paths:

### Test Plan path (preferred)

```bash
USE_TEST_PLANS=1 ./scripts/test-gate.sh fast
```

Uses `-testPlan FastGate` with `xcodebuild`.

### Legacy fallback path

```bash
./scripts/test-gate.sh fast
```

Uses hard-coded `-only-testing:` arrays (pre-migration behavior).

### Gate-to-plan mapping

| Gate | Test Plan path | Fallback path |
|---|---|---|
| `fast` | `-testPlan FastGate` | `-only-testing:` array |
| `proposal-006` | `-testPlan ProviderGate` (unit) + `-only-testing:` (UI) | `-only-testing:` array |
| `full` | `-testPlan FullGate` | `xcodebuild test` (all) |
| `ui-smoke` | **No change** — always `-only-testing:` | XCUITest, not tag-eligible |

### Guardrail

`guard_plan_tag_sync` (in `test-gate.sh`) validates that the `selectedTests` entries in each `.xctestplan` match the authoritative `@Tag` declarations. Run standalone:

```bash
./scripts/test-gate.sh guardrails
```

---

## 9. What Stays on XCTest

| Component | Reason |
|---|---|
| `Chainworks_ForgeUITests.swift` | Swift Testing has no `XCUIApplication` or UI testing equivalent |
| Any future `measure {}` performance tests | Swift Testing has no performance measurement API |
| `TestBundleMarker` (`NSObject` subclass) | `Bundle(for:)` requires `NSObject`; no Swift Testing alternative |

---

## 10. Known Gaps

Residual items from the migration that are not yet resolved:

| Gap | Detail | Severity |
|---|---|---|
| `awaitCondition()` does not use `confirmation()` | The helper uses a manual polling loop instead of the Swift Testing `confirmation()` primitive | Low — functionally equivalent |
| Fixture loader hardening | Some fixture loaders still force-unwrap bundle URLs (`!`) instead of `try #require(...)` | Low — only affects test crash messages |
| `GooseServerTransportTests` local mock | File uses a local `MockURLProtocol: URLProtocol, @unchecked Sendable` class instead of Lane A/B | Medium — `@unchecked Sendable` in test infra |
| Residual `@unchecked Sendable` helpers | `SharedEventCollector` (SharedMocks) and `EventCollector` (LiveGooseConnectionProofTests) remain class-based `@unchecked Sendable` | Medium — compiler-unverified concurrency |
| `full` gate stability | The full gate has observed intermittent SwiftData crash/restart in `WorkflowOrchestrator.currentIteration(for:)` and a UI assertion failure in the live fixture flow | High — blocks `full` gate green status |
