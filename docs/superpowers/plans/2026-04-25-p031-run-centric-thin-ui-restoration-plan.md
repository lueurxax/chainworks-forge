# P031 Run-Centric Thin UI Restoration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the missing P031 operator inspection surfaces in a run-centric workspace while keeping the macOS app read-only and GraphQL-only.

**Architecture:** The macOS app remains a thin reader: SwiftUI asks `P031WorkflowReadStore` for server-owned projections and renders explicit unavailable states when GraphQL does not expose a safe readback. The control-plane owns durable artifact/catalog/workflow readback; the UI must not scan `.chainworks`, artifact files, SwiftData models, MCP, or local workflow services.

**Tech Stack:** SwiftUI, Swift concurrency, P031 GraphQL read boundary, async-graphql Rust server, SQLite-backed projection repositories, `scripts/test-gate.sh`.

---

## File Structure

- Modify `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
  - Extend P031 read models for selected-run idea context, catalog/workflow metadata, stage transition presentation, artifact payload readback, and explicit unavailable reasons.
  - Extend `P031WorkflowReadStore`, `P031GraphQLWorkflowReadStore`, `P031InMemoryWorkflowReadStore`, and `P031ThinWorkflowScreenCoordinator`.
- Modify `Chainworks Forge/Views/RunsHomeView.swift`
  - Restore the run-centric workspace: selected run header, idea context, stage transition visualization, artifacts/report viewer, and catalog/agent context panels.
  - Keep write paths as documented unavailable actions only.
- Modify `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`
  - Add coverage for title/idea context, transition presentation, artifact markdown/JSON rendering contracts, unavailable payload states, and no forbidden GraphQL mutations.
- Modify `control-plane/crates/graphql-server/src/schema.rs`
  - Add read-only GraphQL queries only if existing schema lacks safe artifact/catalog payload readback.
- Modify `control-plane/crates/graphql-server/src/types/artifact.rs`
  - Add typed payload readback only if the server can safely read durable artifact content.
- Modify Rust DB/domain files only if existing repositories do not expose the needed server-owned data.
- Modify docs/evidence only after runtime/gate evidence exists.

## Task 1: Contract Map And Failing Swift Tests

**Files:**
- Modify: `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`
- Read: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- Read: `control-plane/crates/graphql-server/src/schema.rs`

- [ ] **Step 1: Add tests for selected-run idea context**

Add a test that builds `P031InMemoryWorkflowReadStore` with a run whose `ideaID` points to an idea with title/body/status/project key. Assert `P031ThinWorkflowScreenCoordinator.loadRunDetail` exposes:

```swift
#expect(detail.ideaContext.title == "Implement Proposal 031")
#expect(detail.ideaContext.status == "active")
#expect(detail.ideaContext.body.contains("Thin GraphQL-only UI"))
#expect(detail.ideaContext.projectKey == "chainworks")
```

- [ ] **Step 2: Add tests for stage transition presentation**

Add three in-memory stages: completed, blocked, pending. Assert the run detail presentation has ordered transition nodes:

```swift
#expect(detail.stageTransitions.map(\.stageTitle) == [
  "Proposal drafted",
  "Implementation reviewed",
  "Approval required",
])
#expect(detail.stageTransitions[0].connectorState == .completed)
#expect(detail.stageTransitions[1].connectorState == .blocked)
#expect(detail.stageTransitions[2].connectorState == .pending)
```

- [ ] **Step 3: Add tests for artifact viewer contracts**

Add artifacts with `format` values `markdown`, `json`, and `diff`, plus one metadata-only report. Assert the presentation chooses render modes without reading files:

```swift
#expect(detail.artifactViewerRows[0].renderMode == .markdown)
#expect(detail.artifactViewerRows[1].renderMode == .json)
#expect(detail.artifactViewerRows[2].renderMode == .diff)
#expect(detail.artifactViewerRows[3].payloadState == .metadataOnly)
```

- [ ] **Step 4: Run the targeted Swift test and confirm the new tests fail**

Run:

```bash
xcodebuild test -scheme "Chainworks Forge" -destination 'platform=macOS' -only-testing:"Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests" -derivedDataPath /tmp/chainworks-p031-dd
```

Expected: the new tests fail to compile because `ideaContext`, `stageTransitions`, and `artifactViewerRows` do not exist yet.

## Task 2: Swift Read Models And Coordinator

**Files:**
- Modify: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- Test: `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`

- [ ] **Step 1: Add read models**

Add these value types near the existing P031 presentation models:

```swift
struct P031IdeaContextPresentation: Equatable, Sendable {
  let id: String
  let title: String
  let body: String
  let status: String
  let projectKey: String?
  let createdAt: String?
  let archivedAt: String?
}

enum P031StageConnectorState: Equatable, Sendable {
  case completed
  case blocked
  case running
  case pending
  case unavailable
}

struct P031StageTransitionPresentation: Equatable, Sendable {
  let stageID: String
  let stageTitle: String
  let statusText: String
  let attemptText: String
  let connectorState: P031StageConnectorState
  let evidenceLabels: [String]
}

enum P031ArtifactRenderMode: Equatable, Sendable {
  case markdown
  case json
  case diff
  case plainText
  case metadataOnly
  case unavailable
}

struct P031ArtifactViewerPresentation: Equatable, Sendable {
  let artifactID: String
  let title: String
  let subtitle: String
  let renderMode: P031ArtifactRenderMode
  let payloadState: P031ArtifactPayloadAvailability
  let payloadText: String?
  let unavailableReason: String?
}

struct P031CatalogContextPresentation: Equatable, Sendable {
  let workflowID: String
  let workflowTitle: String
  let workflowSnapshotHash: String?
  let catalogSnapshotHash: String?
  let statusText: String
}
```

- [ ] **Step 2: Extend `P031WorkflowReadStore`**

Add read-only methods:

```swift
func fetchIdea(id: String) async throws -> P031IdeaReadModel?
func fetchArtifactPayload(runID: String, artifactID: String) async throws -> P031ArtifactPayloadReadModel
```

If GraphQL payload readback is not implemented yet, `P031GraphQLWorkflowReadStore.fetchArtifactPayload` must return a typed unavailable model from server metadata, not read local files.

- [ ] **Step 3: Extend run detail loading**

Update `P031ThinWorkflowScreenCoordinator.loadRunDetail` to fetch run detail, stages, artifacts, report metadata, idea context, and artifact payload preview data. The selected run detail must still render if idea or payload reads fail; failures become explicit unavailable rows.

- [ ] **Step 4: Run targeted Swift tests**

Run the same targeted Swift test command from Task 1.

Expected: new tests pass, existing P031 tests still pass.

## Task 3: Run-Centric Workspace UI

**Files:**
- Modify: `Chainworks Forge/Views/RunsHomeView.swift`

- [ ] **Step 1: Replace the thin placeholder detail stack**

Keep `NavigationSplitView`, but make selected-run detail order:

1. `P031RunDetailSummaryCard`
2. `P031IdeaContextCard`
3. `P031StageTransitionMapCard`
4. `P031ArtifactViewerCard`
5. `P031CatalogContextCard`
6. `P031ApprovalInboxCard`
7. `P031ReportMetadataCard`
8. `P031DaemonLifecycleCard`

- [ ] **Step 2: Add `P031IdeaContextCard`**

Render title, status, project key, and body excerpt from GraphQL idea data. If absent, show "Idea context unavailable" with the read error text.

- [ ] **Step 3: Add `P031StageTransitionMapCard`**

Render a vertical transition map with fixed-size status markers and connectors. Use the `connectorState` from the presentation model. Do not call old `WorkflowMapProjectionService`.

- [ ] **Step 4: Add `P031ArtifactViewerCard`**

Render artifact list plus selected preview. Use existing `ArtifactContentRenderer` only with explicit payload text supplied by the P031 read model. If payload text is unavailable, show server-provided unavailable reason and metadata.

- [ ] **Step 5: Add `P031CatalogContextCard`**

Render workflow/catalog snapshot hashes and catalog status. If catalog contents are not exposed over GraphQL, show "Catalog snapshot content unavailable through GraphQL" rather than reading files.

- [ ] **Step 6: Build**

Run:

```bash
./scripts/test-gate.sh build
```

Expected: build succeeds with no new Swift warnings from P031 files.

## Task 4: Server Artifact Payload Readback If Needed

**Files:**
- Modify: `control-plane/crates/graphql-server/src/schema.rs`
- Modify: `control-plane/crates/graphql-server/src/types/artifact.rs`
- Modify Rust repository/domain files only when needed.

- [ ] **Step 1: Confirm whether payload readback already exists**

Search GraphQL schema for an existing artifact payload/content query. If absent, add a read-only `artifactPayload(runId: ID!, artifactId: ID!)` query.

- [ ] **Step 2: Add typed payload object**

Expose:

```rust
pub struct GqlArtifactPayload {
    pub artifact_id: ID,
    pub run_id: ID,
    pub format: String,
    pub payload_text: Option<String>,
    pub availability_state: GqlPayloadAvailabilityState,
    pub unavailable_reason_code: Option<GqlPayloadUnavailableReasonCode>,
    pub diagnostic_id: Option<String>,
    pub server_debug_detail: Option<String>,
}
```

- [ ] **Step 3: Enforce server-owned readback only**

The resolver may use canonical artifact metadata/repositories and server-owned artifact paths. It must reject path traversal and run/artifact mismatches. It must never expose arbitrary client paths.

- [ ] **Step 4: Add Rust tests**

Add resolver tests for:

- Markdown artifact returns payload text.
- JSON artifact returns payload text.
- Report metadata returns metadata-only with `PayloadDeferredByP031`.
- Wrong run/artifact pairing returns unavailable/forbidden.

- [ ] **Step 5: Run Rust-focused tests**

Run the narrow Rust package test command identified by the existing test structure. If no narrow test exists, run:

```bash
cd control-plane && cargo test -p graphql-server
```

Expected: artifact payload readback tests pass.

## Task 5: GraphQL Documents And Thin Boundary Enforcement

**Files:**
- Modify: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- Modify: `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`

- [ ] **Step 1: Add GraphQL documents**

Add read-only documents for `idea`, `artifactPayload`, and any catalog context field that exists. Ensure operation names do not contain forbidden write/control tokens.

- [ ] **Step 2: Add decode tests**

Use fake GraphQL transport payloads to decode:

- idea detail with body/status/project key;
- artifact payload markdown;
- artifact payload JSON;
- metadata-only report payload;
- unavailable payload reason.

- [ ] **Step 3: Add boundary tests**

Assert the P031 scanner still rejects mutations and forbidden operation names after adding new documents.

- [ ] **Step 4: Run targeted Swift tests**

Run the targeted P031 Swift test command.

Expected: all P031 Swift tests pass.

## Task 6: Evidence, Dogfood, And Gates

**Files:**
- Modify: `docs/evidence/p031-dogfood-signoff.md`
- Modify: `docs/evidence/p031-ux-accessibility-signoff.md`
- Modify: `docs/evidence/p031-degraded-state-evidence.md`
- Modify: `docs/reference/p031-phase-0-artifact-manifest.json`

- [ ] **Step 1: Run proposal gate**

Run:

```bash
./scripts/test-gate.sh proposal-031
```

Expected: pass.

- [ ] **Step 2: Run readiness gate**

Run:

```bash
./scripts/test-gate.sh proposal-031-readiness
```

Expected: fail only on human signoff items until runtime dogfood evidence is captured.

- [ ] **Step 3: Capture runtime evidence**

Launch the app against the restored daemon/database and capture screenshots showing:

- runs list with recognizable idea titles;
- selected run detail with idea context;
- stage transition visualization;
- artifact markdown or JSON rendering;
- catalog context panel;
- daemon lifecycle panel.

- [ ] **Step 4: Update evidence docs**

Mark completed evidence items only after the corresponding screenshot/test output exists. Keep waiver items explicit if the operator chooses a waiver.

- [ ] **Step 5: Re-run readiness gate**

Run:

```bash
./scripts/test-gate.sh proposal-031-readiness
```

Expected: pass or fail only on intentionally human-owned external signoff that has been explicitly waived.

## Self-Review

- Spec coverage: the plan covers run detail, ideas, catalogs, stage transitions, artifact markdown/JSON rendering, report metadata, daemon lifecycle, dogfood evidence, and P031 GraphQL-only constraints.
- Placeholder scan: no task depends on local file scanning, SwiftData, MCP calls, GraphQL mutations, or old write actions.
- Type consistency: new Swift presentation types are consumed by coordinator tests first, then by SwiftUI cards. Server payload readback is conditional and must be wired through `P031WorkflowReadStore`.
