# Test Suite Architecture

Implemented reference for the Chainworks Forge automated test suite after migration of the unit target from XCTest to Swift Testing.

## Scope

This document describes:

- the current structure of `Chainworks ForgeTests/`
- Swift Testing conventions used by the repository
- concurrency and mock-lane rules
- shared test infrastructure
- tag and test-plan ownership
- what remains on XCTest
- known residual gaps in the suite itself

Operational gate execution lives separately in [test-gates.md](test-gates.md).
Agent-facing execution guidance for previews, remote UI runs, and app-launched proof lives in [agent-ui-test-execution.md](agent-ui-test-execution.md).
For remote macOS UI proof, that document should be read as the authoritative contract for both Codex and Claude Code, with `test@SMacBook.local` as the canonical SSH target.

## Inventory

The unit test target (`Chainworks ForgeTests/`) contains 20 Swift files:

| Category | Count | Notes |
|---|---|---|
| Executable Swift Testing unit tests | 18 | All use `import Testing`, `@Suite` structs |
| Shared helper files | 2 | `TestSupport.swift`, `SharedMocks.swift` — not executable tests |

The UI test target (`Chainworks ForgeUITests/`) remains on XCTest.

### Executable test files

| File | Tags | Mock Lane |
|---|---|---|
| `ArtifactManagerTests` | `.fast` | — |
| `ArtifactValidationTests` | — | — |
| `Chainworks_ForgeTests` | `.fast` | — |
| `EndToEndTests` | `.integration` | Lane A |
| `GooseAgentExecutorTests` | — | Lane B |
| `GooseServerLiveIntegrationTests` | `.live` | — |
| `GooseServerTransportTests` | — | Local mock |
| `GooseSessionBridgeTests` | — | Lane B |
| `GooseStreamEventMapperTests` | — | — |
| `LiveGooseConnectionProofTests` | `.live` | — |
| `LiveProposalWorkflowTests` | `.live` | — |
| `OrchestratorTests` | `.fast` | Lane B |
| `ProviderPlatformTests` | `.fast`, `.provider` | — |
| `ResumeManagerTests` | `.fast` | — |
| `RunPlanCompilerTests` | — | — |
| `SimulatedAgentExecutorTests` | — | Lane A |
| `TransitionEvaluatorTests` | — | — |
| `WorkspaceIsolationTests` | — | — |

### Helper files

| File | Role |
|---|---|
| `TestSupport.swift` | Assertion helpers, fixture loaders, async polling, `TestBundleMarker` |
| `SharedMocks.swift` | `StubGooseTransport`, `ObservableGooseTransport`, `SharedStaticResultExecutor`, `SharedEventCollector` |

## Framework Conventions

Every migrated unit test file follows these conventions:

| Pattern | Convention |
|---|---|
| Import | `import Testing` only |
| Suite declaration | `@Suite("Display Name") struct FooTests` |
| Test function | `@Test("description") func something() async throws` |
| Setup | `init() throws` or `init() async throws` |
| Teardown | Avoided; cleanup is handled by deallocation and local `defer` |
| Assertions | `#expect(...)` |
| Unwrap | `try #require(...)` |
| Failure | `Issue.record(...)` |
| Throws | `#expect(throws: SomeError.self) { ... }` |
| No-throw | `#expect(throws: Never.self) { ... }` |
| Identity | `#expect(x === y)` |

### Assertion Mapping

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

## Concurrency Rules

Swift Testing does not run test functions on the main actor by default.

| Rule | Detail |
|---|---|
| `@MainActor` access | Mark the suite/test `@MainActor` or wrap access in `await MainActor.run {}` |
| Serialization | Suites with shared mutable state use `.serialized`; independent suites stay parallel |
| Sendable | Shared test doubles remain `Sendable` regardless of suite serialization |

Suites currently using `.serialized`:

- `OrchestratorTests`
- `ResumeManagerTests`
- `WorkspaceIsolationTests`

## Mock Strategy

The suite uses two explicit transport mock lanes.

### Lane A: `StubGooseTransport`

`Sendable` struct for tests that only need deterministic stimulus injection and do not assert on transport-side effects.

Used by:

- `SimulatedAgentExecutorTests`
- `EndToEndTests`
- other stream-only paths

### Lane B: `ObservableGooseTransport`

`actor` for tests that assert on request content, session lifecycle, and call counts.

Used by:

- `GooseAgentExecutorTests`
- `GooseSessionBridgeTests`
- `OrchestratorTests`

### Lane Assignment

| Test file | Lane | Reason |
|---|---|---|
| `GooseAgentExecutorTests` | B | Asserts on execution policy and session closure |
| `GooseSessionBridgeTests` | B | Asserts on session lifecycle |
| `OrchestratorTests` | B | Asserts on request propagation and call counts |
| `SimulatedAgentExecutorTests` | A | Stateless result injection |
| `EndToEndTests` | A | Stateless result injection |
| `GooseServerTransportTests` | Local mock | Uses local `MockURLProtocol` path |

## Shared Test Infrastructure

### `TestSupport.swift`

Key helpers:

| Helper | Purpose |
|---|---|
| `expectRunCompleted(_:)` | Asserts `run.status == .completed` with diagnostic dump |
| `expectRunBlocked(_:)` | Asserts `run.status == .blocked` |
| `expectRunWaitingApproval(_:)` | Asserts `run.status == .waitingApproval` |
| `expectArtifactExists(_:in:)` | Asserts named artifact exists in the run collection |
| `expectArtifactNonEmpty(_:in:workspace:)` | Asserts a persisted artifact exists and is non-empty |

Other infrastructure:

- `awaitCondition(...)` for bounded polling
- `TestBundleMarker` for fixture bundle lookup
- fixture loaders such as `loadTestCanonicalWorkflow()`, `loadTestCanonicalCatalog()`, `makeTestModelContainer()`

### `SharedMocks.swift`

| Component | Type | Role |
|---|---|---|
| `StubGooseTransport` | `struct` | Lane A stimulus injection |
| `ObservableGooseTransport` | `actor` | Lane B observable transport |
| `SharedStaticResultExecutor` | `struct` | Static-result E2E injection |
| `SharedEventCollector` | class | Shared event collection helper |

### `TestTags.swift`

Tag extensions remain the authoritative categorization layer for the unit suite:

- `.fast`
- `.smoke`
- `.integration`
- `.live`
- `.provider`

## Parameterized Tests

The migration consolidated several repetitive suites with `@Test(arguments:)`.

### `TransitionEvaluatorTests`

Parameterizes approval and condition evaluation cases that used to be split across many near-identical methods.

### `GooseStreamEventMapperTests`

Parameterizes ignored-event JSON fixtures into a single mapping test.

### `ResumeManagerTests`

Uses parameterized coverage for resume classification scenarios.

## Tags and Test Plans

Tags are the source of truth for gate membership. Plans in `TestPlans/` exist as repository metadata and sync guardrails for `selectedTests`, not as the only trustworthy execution surface for agent runs.

| Tag | Purpose | Suites |
|---|---|---|
| `.fast` | Inner-loop engineering gate | `Chainworks_ForgeTests`, `OrchestratorTests`, `ProviderPlatformTests`, `ResumeManagerTests`, `ArtifactManagerTests` |
| `.smoke` | UI-level smoke coverage (unit-side only) | — |
| `.integration` | External provider connectivity | `EndToEndTests` |
| `.live` | Running Goose server required | `GooseServerLiveIntegrationTests`, `LiveGooseConnectionProofTests`, `LiveProposalWorkflowTests` |
| `.provider` | Provider-platform slice | `ProviderPlatformTests` |

| Plan | Tag filter | Targets |
|---|---|---|
| `FastGate.xctestplan` | `.fast` | `Chainworks ForgeTests` |
| `ProviderGate.xctestplan` | `.provider` | `Chainworks ForgeTests` |
| `FullGate.xctestplan` | All | `Chainworks ForgeTests` + `Chainworks ForgeUITests` |

Important execution note:

- `xcodebuild -testPlan ...` against Swift Testing targets has already shown non-proving `0`-test outcomes on current toolchains.
- The canonical agent execution path is still the repository gate runner, which defaults to targeted test IDs and only opts into plans when explicitly requested.
- Agents should not treat a green raw test-plan run with `0` executed tests as valid evidence.

Gate execution and plan synchronization rules are documented in [test-gates.md](test-gates.md).

## What Stays on XCTest

| Component | Reason |
|---|---|
| `Chainworks_ForgeUITests.swift` | Swift Testing has no `XCUIApplication` equivalent |
| Future `measure {}` performance tests | Swift Testing has no direct performance API |
| `TestBundleMarker` | `Bundle(for:)` still requires `NSObject` |

## Known Gaps

Residual gaps in the suite itself:

| Gap | Detail | Severity |
|---|---|---|
| `awaitCondition()` does not use `confirmation()` | Uses manual polling rather than Swift Testing confirmation primitives | Low |
| Fixture loader hardening | Some loaders still force-unwrap bundle URLs | Low |
| `GooseServerTransportTests` local mock | Uses local `MockURLProtocol: URLProtocol, @unchecked Sendable` instead of Lane A/B | Medium |
| Residual `@unchecked Sendable` helpers | `SharedEventCollector` and `EventCollector` remain compiler-unverified | Medium |
| `full` gate stability | Full suite has historically observed SwiftData/UI instability outside the active slice | High |
| Raw `.xctestplan` proving quality | Some direct `xcodebuild -testPlan ...` invocations can return green `0`-test results against Swift Testing suites | High |

## Agent Execution Boundary

The test suite structure alone is not enough to decide how agents should validate UI work.

- XCTest UI smoke remains the default surface-proof path.
- Preview review remains the default design-review path.
- Proposal-level repo-backed delivery sign-off may require an app-launched harness inside the app process, not just `Chainworks ForgeUITests`.

Operational rules for choosing between those paths, including remote-host execution and app-launched dogfood proof, are documented in [agent-ui-test-execution.md](agent-ui-test-execution.md).

Important:

- the structure of the suite does not authorize local UI execution
- repository-enforced remote-only UI policy still applies
- if an agent needs shell/operator proof, it should use the documented remote path rather than infer one from test-plan names alone
- some proposal gates now satisfy app-level proof through a remote app-launched export mode, not through a local UI test runner

## Related Docs

- [test-gates.md](test-gates.md)
- [agent-ui-test-execution.md](agent-ui-test-execution.md)
- [workflow-execution-engine.md](workflow-execution-engine.md)
- [provider-platform.md](provider-platform.md)
