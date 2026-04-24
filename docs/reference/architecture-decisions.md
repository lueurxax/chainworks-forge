# Architecture Decisions

This document records the key architecture decisions made during the foundation layer implementation (domain model + YAML DSL parser). Each decision is numbered for traceability.

## ARCH-001: Explicit CodingKeys for snake_case YAML

**Context:** Canonical YAML files use `snake_case` keys (`schema_version`, `backend_profile`). Swift structs use `camelCase` properties. Yams' `YAMLDecoder` does not have a built-in `keyDecodingStrategy` like `JSONDecoder`.

**Decision:** All Codable structures use explicit `CodingKeys` enums to map `snake_case` YAML keys to `camelCase` Swift properties. Types with only single-word keys omit `CodingKeys`.

**Consequence:** Every YAML-facing struct must maintain its `CodingKeys` when fields are added. Parser tests that decode the canonical `examples/` files verify correctness.

## ARCH-002: Single active run per idea

**Context:** The MVP requires `single_active_run_per_idea: true`, but SwiftData's `Idea.runs` relationship is an unconstrained collection. Without protection, concurrent runs for the same idea could be created.

**Decision:** `RunRepository` is the sole approved entry point for creating `Run` instances. It atomically checks for active runs before inserting. `@MainActor` serialization prevents TOCTOU races.

**Consequence:** Direct `Run(...)` construction outside `RunRepository` is a contract violation, enforced by automated codebase scan tests and CI grep guards.

## ARCH-003: Drift detection on Run

**Context:** When YAML files change between app sessions, a resumed run might execute against a different workflow or catalog than it was started with.

**Decision:** `Run` stores drift detection fields: `driftDetectedAt`, `driftDetails`, `driftDecision`. When drift is detected, the run transitions to `.blocked` status until the engineer chooses a `DriftDecision`.

**Consequence:** The drift-review UI is a runtime concern implemented separately. The domain model provides the necessary fields and state transitions.

## ARCH-004: Full snapshot storage, not hash-only

**Context:** SHA-256 hashes detect drift but cannot reconstruct the original definitions for `continueWithOriginal` resume.

**Decision:** `Run` stores full serialized JSON snapshots (`workflowSnapshotJSON`, `catalogSnapshotJSON`) alongside their hashes. Hashes enable quick comparison; snapshots enable resuming with the exact original definitions.

**Consequence:** Run records are larger, but drift recovery is complete. Snapshots are `Data` blobs in SwiftData, not separate models.

## ARCH-005: Integer cents for cost tracking

**Context:** `Double` arithmetic accumulates floating-point precision errors when summing many small agent costs.

**Decision:** All costs are stored as `Int64` minor currency units (cents). `$12.34` is stored as `1234`. Rounding to display format happens only at the presentation layer.

**Consequence:** No precision drift in aggregation. All cost-related code works with integers.

## ARCH-009: Compact workflow CodingKeys

**Context:** `CompactWorkflowMeta` uses the `required_providers` key, which needs explicit `CodingKeys` mapping.

**Decision:** `CompactWorkflowMeta` has `CodingKeys` for `requiredProviders`. Other compact types (`CompactStage`, `CompactGate`) use only single-word keys and omit `CodingKeys`.

## ARCH-010 / ARCH-011: Compact workflow is inspector-only

**Context:** The compact workflow format (`proposal-to-release.yaml`) uses hyphenated agent aliases (`proposal-writer`) that do not match canonical catalog IDs (`proposal_writer`). It also omits execution-critical fields: `AgentTask.task`, inputs, outputs, `Transition.when` expressions, loop counters, scoring rules, and failure policy.

**Decision:** Compact workflows are **inspector-only** in the foundation layer. They:
- Parse into `CompactWorkflowDefinition` (type-safe Codable)
- Display in the Workflow Inspector as "Compact Preview" with a "preview only, not executable" label
- Undergo structural-only validation (unique IDs, needs references, no cycles)
- Are **not** normalized to `WorkflowDefinition`
- Are **not** validated against the agent catalog

**Consequence:** Compact-to-executable compilation (alias resolution, task derivation, IO binding) is deferred to the workflow execution engine implementation.

## ARCH-PA-002: Derived currentStageID

**Context:** Storing `currentStageID` as a persistent field on `Run` creates divergence risk across retries, skips, blocked states, and resume.

**Decision:** `Run.currentStageID` is a computed property derived from `stageExecutions`, not a stored field. Priority: `running` > `waitingApproval` > `blocked` > `ready` > last `completed`.

**Consequence:** Single authoritative source of truth for current stage. No synchronization bugs between a stored field and the execution collection.

## ARCH-PA-003: Single-target enforcement

**Context:** The app is a single Xcode target organized by folders, not separate Swift modules/packages. `internal` access control does not provide a real boundary.

**Decision:** Enforcement of the `RunRepository` contract is automated, not structural:
1. `@MainActor` serialization prevents races
2. `testNoDirectRunConstruction` recursively scans all `.swift` files for unauthorized `Run(` construction
3. CI grep guard blocks direct insertion at commit time

**Consequence:** Honest about the limitation. Automated checks catch violations early without over-claiming structural enforcement.

## Canonical serialization for provenance hashing

**Context:** Dictionary types like `[String: WorkflowState]` produce non-deterministic key ordering in JSON serialization, causing false drift detection.

**Decision:** `DefinitionHasher.canonicalEncoder` uses:
- `.sortedKeys` — lexicographic key ordering for deterministic output
- `.withoutEscapingSlashes` — consistent slash handling
- `.iso8601` — deterministic date encoding

**Consequence:** Same object always produces the same JSON bytes and the same SHA-256 hash. Verified by tests that encode the same object 100 times.

## ARCH-031: Thin GraphQL-Only UI Rewrite

**Context:** The macOS operator app traditionally read from SwiftData and issued commands through multiple paths (MCP, local services). This created ambiguity about the source of truth and allowed the UI to become a second control plane.

**Decision:** The macOS UI is narrowed to a thin, GraphQL-only read client.
1. Governed SwiftUI surfaces render workflow truth from server-owned GraphQL projections only.
2. The UI is read-only: no MCP writes, no GraphQL mutations, no local mutation fallback.
3. Every removed write control (Start, Cancel, Retry, Resolve Approval) is replaced with diagnostic guidance and identifiers for external CLI/MCP workflows.
4. UI state is limited to presentation, server-derived caches, read-refresh state, and freshness handling.

**Consequence:** Single authoritative read plane (GraphQL). Commands move outside the macOS UI to validated external workflows. Static guards and a machine-readable UI inventory enforce the read-only boundary.
