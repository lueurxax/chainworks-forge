# Test Gates

Chainworks Forge uses layered test gates instead of one default `xcodebuild test` loop for every change.

This document is operational: it describes which gate to run and why. Structural details of the migrated Swift Testing suite live in [test-suite-architecture.md](test-suite-architecture.md).
Agent-specific UI execution rules, including remote-host and app-launched proof guidance, live in [agent-ui-test-execution.md](agent-ui-test-execution.md).
For remote macOS UI/app proof, the canonical SSH target is `test@SMacBook.local`.

The purpose is simple:

- keep the fast inner loop fast
- isolate expensive UI automation from core runtime validation
- keep proposal-specific proof slices reproducible
- reserve the full suite for sign-off, not every edit

## Entry Point

Use the repository gate runner:

```bash
./scripts/test-gate.sh list
```

The runner does three things before every gate:

- refuses to start if build/test tooling is already running
- for `ui-smoke`, `proposal-006`, and `full`, also refuses to start if `Chainworks Forge.app` is already running on the host
- prints the latest known `Chainworks Forge-*.ips` crash log path
- reports a newly created crash log path when a gate fails

The runner is also the canonical proving path for agents. Direct `xcodebuild -testPlan ...` invocations are allowed for diagnostics, but they are not the default evidence path because current Swift Testing toolchains can still yield green `0`-test outcomes for raw plan execution.

For Codex and Claude Code, gate execution should normally happen through:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh <gate>"
```

Do not omit the `test@` user when writing prompts, docs, or runbooks for remote UI work.

## Gate Layers

### `guardrails`

Cheapest possible structural gate.

Scope:

- no direct `Run(...)` construction outside `RunRepository`

Use when:

- editing persistence, repositories, constructors, or test scaffolding

Command:

```bash
./scripts/test-gate.sh guardrails
```

### `build`

Compile-only gate.

Scope:

- source guardrails
- app build

Use when:

- touching broad cross-cutting code and you want a fast compile sanity pass first

Command:

```bash
./scripts/test-gate.sh build
```

### `fast`

Default inner-loop engineering gate.

Scope:

- source guardrails
- app build
- high-ROI unit/runtime slices:
  - `ProviderPlatformTests`
  - `OrchestratorTests`
  - `ResumeManagerTests`
  - `ArtifactManagerTests`
  - `RunTests`

Use when:

- changing models, orchestration, provider resolution, resume, or artifact persistence

Command:

```bash
./scripts/test-gate.sh fast
```

Important:

- this is the proving path for the fast lane
- do not substitute it with raw `xcodebuild -testPlan FastGate test` and assume the result is equivalent

### `ui-smoke`

Focused operator-shell UI smoke gate.

Scope:

- approval inbox reachability
- approval gate surface
- diagnostic start run placeholder
- missing-runtime recovery guidance
- run progress surface

Use when:

- changing navigation, shell layout, approvals, start-run flow, or progress UI

Host policy:

- remote-only
- the gate runner refuses to execute this gate outside the approved UI host list

Command:

```bash
./scripts/test-gate.sh ui-smoke
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"
```

### `proposal-006`

Provider-platform gate for settings/diagnostics/readiness work.

Scope:

- `ProviderPlatformTests`
- `testProviderSettingsWizardFlowSurface`
- `testProviderSettingsExportSurface`
- `testPilotReadinessRefreshSurface`

Use when:

- changing provider-platform implementation or sign-off evidence

Host policy:

- remote-only because this gate includes UI tests

Command:

```bash
./scripts/test-gate.sh proposal-006
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-006"
```

Important:

- the repository supports `ProviderGate.xctestplan` as metadata, but the canonical agent path still runs targeted tests by default

### `proposal-012`

UI-quality proof gate for the implemented visual-polish and bounded accessibility slice.

Scope:

- runtime proof for `WorkflowMapView`
- runtime proof for `ReleaseGateView`
- explicit `1024×768` minimum-window proof
- bounded accessibility proof for:
  - Differentiate Without Color
  - Increase Contrast
  - Reduce Transparency
  - accessibility tree / focus order

Use when:

- reproving the implemented UI quality slice on the current head
- validating the bounded adopter slice and secondary runtime owner surfaces beyond preview/code evidence
- collecting same-head screenshot-bearing proof for UI quality audits

Host policy:

- remote-only because this gate is UI automation

Command:

```bash
./scripts/test-gate.sh proposal-012
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-012"
```

### `proposal-014`

Design-system and brand-application proof gate for the implemented Forge visual rollout.

Scope:

- shell brand header visibility
- foreground attention banner proof
- approval / progress / recovery continuity on branded surfaces
- provider/setup owner surfaces included in the bounded visual rollout
- workflow map / release gate / min-window / adopter accessibility owners carried forward into the branded proof lane

Use when:

- reproving the implemented design-system and brand-application slice on the current head
- collecting approved-host same-head proof for shell/run/setup/recovery visual adoption
- validating that the branded rollout still preserves accessibility and recovery owner execution

Host policy:

- remote-only because this gate is UI automation

Command:

```bash
./scripts/test-gate.sh proposal-014
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-014"
```

Important:

- the gate keeps its historical proposal label for reproducibility
- the documentation source of truth for the slice is now [design-system-and-brand-application.md](design-system-and-brand-application.md), not the old proposal file

### `proposal-017` Retained Historical Alias

Retained gate alias for workflow-authority, conflict-truth, and lead-mediation behavior.

Scope:

- `TransitionAuthorityResolver` and `CandidateTransitionEvaluation` parity fixtures (Swift/Rust)
- blocking conflict persistence by fingerprint
- non-blocking advisory rejection history
- `ImplementationHandoffStatus` authority and handoff-failure readback
- aggregate field authority for `proposal_review_summary_v1`
- fail-closed unknown transition-input classification
- per-surface `workflow_conflict` report shape parity (Swift/MCP/GraphQL)
- transition cursor and resume consistency with conflict truth
- **Lead Conflict Mediation**: Lead selection, mediation lifecycle,
  owner-aware execution, and settlement validation.
- **Mandatory Lead Validation**: Static workflow validation and runtime
  preflight for lead resolution.
- Phase B dogfood exit evidence, Phase C external catalog enforcement
  inventory, known-issues migration record validation, and workflow-conflict
  rollout metric fixtures.

Use when:

- changing workflow authority, transition evaluation, or conflict/advisory persistence
- changing implementation-entry handoff or approved proposal freeze logic
- changing lead mediation selection, lifecycle, or settlement
- changing Phase C lead-validation or runtime preflight logic
- changing report/API shapes for workflow conflicts or mediation records

Host policy:

- local target; exercises Swift and Rust workspace tests without a UI target

Command:

```bash
./scripts/test-gate.sh proposal-017 # retained historical alias
```

Important:

- this gate is the canonical proof path for [workflow-execution-engine.md](workflow-execution-engine.md) (authority/conflict/mediation) and [execution-truth-and-recovery.md](execution-truth-and-recovery.md) (conflict recovery/handoff)
- it validates that agent-authored `next_stage` cannot override the compiled graph
- it validates the implemented D4F404B7-class replay outcome across Swift and Rust for the full workflow-conflict slice
- it proves Rust `agent_executions` owner-kind migration, lead mediation runtime, and lead-validation requirements
- it verifies the workflow-conflict evidence bundle under `docs/reference/workflow-conflict-evidence/`,
  including flag-gated dogfood closeout and external catalog attestation

### Phase 0 Contract Freeze

The `proposal-017` retained historical alias verifies the existence and approval status of retained backend contract artifacts before the gate proceeds. These artifacts are the canonical sources of truth for major migration seams:

- **Approval Mediation**: `docs/reference/workflow-conflict-evidence/phase-0-approval-mediation-contract.json`
- **Execution Identity**: `docs/reference/workflow-conflict-evidence/phase-0-mediation-execution-identity-contract.md`
- **Work Item Owner**: `docs/reference/workflow-conflict-evidence/phase-0-work-item-execution-owner-contract.json`
- **Lead Resolver**: `docs/reference/workflow-conflict-evidence/phase-0-phase-b-lead-resolver.json`
- **Settlement Boundary**: `docs/reference/workflow-conflict-evidence/phase-0-settlement-service-boundary.md`
- **Dogfood Exit Record**: `docs/reference/workflow-conflict-evidence/phase-b-dogfood-exit-record.json`
- **Phase C Inventory**: `docs/reference/workflow-conflict-evidence/phase-c-external-catalog-enforcement-inventory.json`
- **Known Issues Migration Records**: `docs/reference/workflow-conflict-evidence/phase-a-known-issues-migration-records.json`

### `proposal-019`

Context-strategy framework gate for strategy handoff, lazy evidence, telemetry, and recommendation proof.

Scope:

- `Proposal019Tests`
- `RuntimeSessionBridgeTests`
- `RuntimeAgentExecutorTests`
- `OrchestratorTests`

Use when:

- reproving the implemented context-strategy slice on the current head
- validating lazy-evidence retrieval and tier-escalation behavior
- verifying canonical strategy telemetry and recommendation proof owners

Host policy:

- local macOS gate
- this is a named repository-owned proof lane, not just an ad-hoc focused `xcodebuild test`

Command:

```bash
./scripts/test-gate.sh proposal-019
```

Important:

- this gate is the canonical proof path for the implemented strategy slice
- the stable documentation source of truth for the slice is now [context-strategy-and-experiment-framework.md](context-strategy-and-experiment-framework.md), not the old proposal file

### `proposal-022`

Proposal-loop fidelity gate for review-corpus persistence, score-lift backlog truth, and targeted rereview proof.

Scope:

- `Proposal022Tests`
- `Proposal022ScaffoldingTests`
- remote app-launched Proposal 022 proof export from the built app

Use when:

- reproving the implemented proposal-loop fidelity slice on the current head
- validating canonical `review_corpus_bundle`, merge provenance, backlog coverage, and targeted rereview truth
- collecting the app-launched proof artifact required by Proposal 022 without depending on local UI execution

Host policy:

- remote-only because this gate includes an app-launched proof step on the approved UI host
- do not run this gate locally after the operator has forbidden local UI/app launches

Command:

```bash
./scripts/test-gate.sh proposal-022
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-022"
```

Important:

- the canonical Proposal 022 app-level proof is no longer a local XCUITest assumption
- the gate builds locally on the remote host, runs the focused non-UI slice, then launches the built app in a deterministic proof-export mode
- pull back the emitted Proposal 022 result JSON after the run if the audit needs inspectable app-proof evidence

### `proposal-024`

Run-surface information architecture gate for segmented shells, focused timeline ownership, and hierarchical artifact browsing proof.

Scope:

- `Proposal024RunSurfaceTests`
- `RunArtifactHierarchyBuilderTests`
- approved-host UI proof for focused timeline and completed-run export continuity owners

Use when:

- reproving the implemented segmented run-surface slice on the current head
- validating deterministic pane routing, shared artifact hierarchy, and repo-backed continuity after metadata demotion
- collecting approved-host UI proof for the subordinate focused-timeline owner path

Host policy:

- remote-only because this gate includes the UI target
- same-head proof should be treated as canonical only when the approved-host workspace matches the tree under review

Command:

```bash
./scripts/test-gate.sh proposal-024
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-024"
```

Important:

- the gate keeps its historical proposal label for reproducibility
- the stable documentation source of truth for the slice is now [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md), not the old proposal file

### `proposal-027`

Rust + SQLite local control-plane daemon gate. Runs the full `control-plane` Rust workspace test suite, covering:

- SQLite repository layer (ideas, runs, stages, approvals, artifacts)
- Projection rebuild and parity verification (`run_summaries`, `stage_summaries`, `approval_inbox`, `artifact_index`)
- Domain engine transitions and command handler semantics (approve, reject, retry, cancel)
- RecoveryService startup-repair for stuck-Running stages

Scope:

- `control-plane` Rust workspace (`cargo test --workspace`)

Use when:

- validating the daemon compiles and all integration tests pass on the current head
- proving projection-layer parity after a run/stage mutation
- confirming approval/retry command semantics match the app-owned baseline
- reproving startup-repair recovery semantics

Host policy:

- local Rust toolchain required; no iOS/macOS simulator needed
- executes in-process with SQLite in-memory databases; no daemon process required

Command:

```bash
./scripts/test-gate.sh proposal-027
```

Important:

- the artifact content rendering slice (formerly `proposal-027`) has been moved to `proposal-027r`
- the stable documentation source of truth for the Rust control-plane is [rust-control-plane.md](rust-control-plane.md)

### `proposal-027r`

Artifact content rendering gate for the unified read-only Markdown/JSON artifact presentation slice (legacy `proposal-027` renderer gate, retained for reproducibility).

Scope:

- `Proposal027Tests`

Use when:

- reproving the implemented unified renderer on the current head
- validating payload-rescue intent, image-safety policy, and parse fallback behavior for artifact content
- confirming JSON tree behavior and markdown document rendering contracts

Host policy:

- local target only; this gate executes unit tests without a UI target

Command:

```bash
./scripts/test-gate.sh proposal-027r
```

Important:

- the stable documentation source of truth for this slice is [artifact-content-rendering.md](../reference/artifact-content-rendering.md)

### `proposal-033`

ACP-only runtime architecture gate for the post-Goose canonical transport slice.

Scope:

- prerequisite `proposal-029` second-wave ACP runtime lane
- `Proposal033Tests`
- `RuntimeSessionBridgeTests`
- `LiveACPConnectionProofTests`
- `MVPGoldenRunTests`
- `ProviderPlatformTests`

Use when:

- proving the ACP-only runtime architecture on the current head
- validating provider-settings migration from Goose-era payloads
- validating ACP-only MCP/session/executor behavior
- verifying operator/runtime docs and gate ownership have caught up with the code

Host policy:

- local target only; this is a focused runtime/unit proof lane without a UI target

Command:

```bash
./scripts/test-gate.sh proposal-033
```

Important:

- this gate hard-depends on `proposal-029`
- `proposal-033` is the repo-owned proof lane for [acp-runtime-transport.md](acp-runtime-transport.md)

### `proposal-037`

ACP execution supervision and idle-watchdog gate.

Scope:

- focused `RuntimeAgentExecutorTests` watchdog and mutation-integrity cases
- focused `OrchestratorTests` durable materialization and same-stage retry lineage cases
- focused `ResumeManagerTests` stalled-boundary grace and reconcile cases
- `RecoveryCoordinatorTests`
- `Proposal013Tests`
- `Proposal019Tests`
- `LiveProposalWorkflowTests`
- `WorkflowMapProjectionTests`
- `RunTimelineInspectorViewTests`

Use when:

- changing ACP watchdog classification, retry ownership, or execution supervision truth
- changing mutation-side-effect verification or Codex receipt telemetry
- changing durable same-stage retry lineage for watchdog-driven retries

Host policy:

- local target only; this is a focused runtime/unit proof lane without a UI target

Command:

```bash
./scripts/test-gate.sh proposal-037
```

Important:

- this gate is the repo-owned proof lane for [037-acp-execution-supervision-and-idle-watchdog.md](../proposals/037-acp-execution-supervision-and-idle-watchdog.md)
- it intentionally targets the explicit P037 proof cases instead of unrelated legacy retry/resume debt in broader suites
- use it instead of ad hoc targeted test mixes when reproving watchdog behavior

### `proposal-041|p041`

Server parity harness, golden fixtures, and behavioral diff gate.

Scope:

- P041 `GoldenRunFixture` inventory and schema validation
- capture/regeneration lifecycle validation through `scripts/parity/capture-golden-run.sh --validate`
- deterministic offline replay over every required P041 fixture
- retained fixture replay databases under `control-plane/target/parity/{fixture_id}/parity.sqlite`
- persisted `BehavioralDiffReport` generation under `control-plane/target/parity/reports/{fixture_id}/`
- persisted `server-replay.json` generation under `control-plane/target/parity/{fixture_id}/`
- fixture-bound `surface_comparisons` for canonical state, projections, GraphQL readback, MCP report readback, artifact identity, and operator summary
- fail-closed shadow side-effect policy for stubbed runtime/provider inputs, including correlated `mode=live_shadow` reports under `control-plane/target/parity/shadow/reports/{fixture_id}/`
- fixture-bound GraphQL run/stage/artifact/projection readback parity via `proposal_041_graphql_readback_parity_surfaces`
- fixture-bound MCP `reports.get` and `report://{run_id}` readback parity via P041-named tests
- P031 handoff parity readiness is tracked through `docs/reference/p031-phase-0-artifact-manifest.json`; do not use obsolete proposal-local evidence paths.

Use when:

- changing server/client parity fixture contracts
- changing Rust replay, projection, artifact/report, GraphQL-readback, or MCP-readback parity surfaces
- preparing P031 thin-client cutover evidence
- updating P041 golden fixtures or behavioral diff schema

Host policy:

- local Rust toolchain required
- no macOS UI target, simulator, daemon process, or live ACP/provider adapter required
- offline replay runs in retained per-fixture SQLite databases and isolated temporary artifact roots

Command:

```bash
./scripts/test-gate.sh proposal-041
```

Important:

- `p041` is accepted as an alias
- the gate fails closed on missing fixtures, invalid schema, missing capture/regeneration provenance, missing executable frozen input references, missing fixture-bound surface comparisons, missing GraphQL/MCP collector owners, missing live-shadow correlation, blocking divergences, or missing P031 handoff evidence
- the P031 handoff artifact is not a substitute for the gate; it is valid only when this gate passes on the same tree

### `proposal-043|p043`

GraphQL projection read contract gate for the thin macOS client.

Scope:

- focused `graphql-server` test slice for the P043 read-contract filters starting with `proposal_043_`
- reference contract validation at `docs/reference/query-projections-and-client-consumption-contract.md`
- matrix coverage for all P043 surfaces: runs home, run detail, stage list / progress, stage detail, approval inbox, artifact viewer, report viewer, runtime health, and experiment comparison
- operator-only V1 scope, exact freshness budget rows, explicit projection freshness fields, freshness behavior limitations, subscription posture, P031/P043 delta policy, GraphQL field proof, projection parity, known gaps, and cutover decision evidence

Use when:

- changing GraphQL read projections or client consumption rules for the thin macOS client
- updating the P031 consumption contract for read surfaces, freshness, or deferred UI states
- reproving the P043 read-contract lane without touching UI or daemon lifecycle implementation

### `proposal-031|p031`

Proposal 031 thin GraphQL-only UI inventory, static guard, and write-path guide gate.

Scope:

- Python script validation (`scripts/p031-thin-ui-gate.py`) for governed UI constraints
- verification of `docs/reference/p031-thin-ui-inventory.json`, `docs/reference/p031-operator-write-path-guide.json`, and `docs/reference/p031-phase-0-artifact-manifest.json`
- rust tests for `proposal_031_` prefix in `graphql-server`
- rust authorization tests `proposal_031_authorization` in `graphql-server`

Use when:

- changing the GraphQL-only thin macOS client boundary
- updating the UI inventory, operator write-path guide, or Phase 0 artifact manifest
- modifying GraphQL read/subscription authorization boundaries for P031 UI

Host policy:

- Python 3 and local Rust toolchain required
- no macOS UI target, simulator, or live daemon required

Command:

```bash
./scripts/test-gate.sh proposal-031
```

Important:

- `p031` is accepted as an alias
- the gate fails closed if governed Swift/GraphQL files violate the GraphQL-only read boundary plus the approval-only mutation exception, or if required Phase 0 artifacts are missing or invalid
- later Phase 0d / Phase 3 evidence entries may remain blocked while implementation is in progress, but the manifest must not mark blocked evidence as `ready`
- dogfood sign-off evidence is outside this gate; this gate only requires the Phase 3 dogfood checklist artifact to exist

### `proposal-072|p072`

Historical gate alias for the implemented UI action boundary: approval-only GraphQL UI mutation boundary and MCP-only command routing.

Scope:

- composes `proposal-031`
- Swift unit tests for the governed GraphQL request boundary, including `approveApproval` / `rejectApproval`
- Rust domain/auth tests for the UI action routing registry and principal surface policies
- Rust GraphQL tests proving `ui_operator` can execute only approval mutations and is denied non-approval command mutations
- docs/inventory checks proving P031 is reconciled with the approval-only exception

Use when:

- changing governed SwiftUI approval actions
- changing GraphQL mutation authorization or principal surface policies
- changing the P031/static UI action boundary or inventory
- changing docs that describe whether UI approvals are diagnostic-only or actionable

Host policy:

- Python 3, local Rust toolchain, and local macOS Swift test toolchain required
- no live daemon, Xcode UI tests, simulator, GitHub PR review, or network access required

Command:

```bash
./scripts/test-gate.sh proposal-072
```

Important:

- `p072` is accepted as an alias; the historical gate name is retained for stable automation compatibility
- non-approval operator commands remain MCP-only for governed UI
- the gate intentionally does not inspect or require GitHub/Copilot PR review disposition
- SwiftUI may use GraphQL mutations only for `approveApproval` and `rejectApproval`

### `proposal-031-readiness|p031-readiness`

Proposal 031 closeout readiness gate.

Scope:

- composes `proposal-031`
- verifies release evidence files are tracked, including report-payload runtime JSON and sanitized degraded-state screenshot evidence
- fails while the Phase 0 manifest, degraded-state evidence, freshness evidence, UX/accessibility evidence, or Phase 3 dogfood sign-off still contain pending/template/limitation states
- fails while the Phase 3 dogfood checklist has unchecked items

Use when:

- deciding whether P031 can be considered implementation-closeout ready
- re-entering implementation approval after dogfood/sign-off evidence is attached

Host policy:

- same as `proposal-031`
- no live daemon required
- expected to fail until Phase 3 dogfood and release-owner sign-off are complete

Command:

```bash
./scripts/test-gate.sh proposal-031-readiness
```

Important:

- this is intentionally stricter than `proposal-031`
- `proposal-031` passing means the static/API/read-boundary contract is intact
- `proposal-031-readiness` passing means P031 release/closeout evidence is no longer known-pending


Host policy:

- local Rust toolchain required
- no macOS UI target, simulator, daemon process, or live ACP/provider adapter required

Command:

```bash
./scripts/test-gate.sh proposal-043
```

Important:

- `p043` is accepted as an alias
- the gate runs `cd control-plane && cargo test -p graphql-server proposal_043_ -- --nocapture` and then validates the reference contract from the repository root
- the test slice covers projection-backed run/stage queries, explicit missing-projection `projectionLag` state, projection-enriched run/stage subscription payloads, `approvalResolved`, and operator-only V1 reads
- it fails closed if the reference contract is missing, not `Implemented` / `Ready with Risks`, omits any required P043 surface, omits exact freshness budget rows, omits projection freshness field proof, omits subscription posture, omits freshness behavior limitations, or omits operator-only V1, projection parity, known holds, or cutover decision coverage
- it is not enough for the GraphQL tests to pass; the reference contract must also validate on the same tree

### `proposal-044`

Post-approval task execution and release gate completion gate.

Scope:

- N-phase sequential ordering for `sequence` and multi-task `then` blocks
- post-approval effective-task resolution and N-phase enqueuing
- end-state task execution before run completion
- multi-task `then` ordering (state_9: auditor → prepush → aggregation)
- no regression on single-task `then` settlement (state_4)
- no regression on simple manual gates (state_3, state_6)
- worktree safety for post-approval release tasks

Command:

```bash
./scripts/test-gate.sh proposal-044
```

### `proposal-045`

Deterministic release operations gate.

Scope:

- frozen `delivery_configuration_json` input-path persistence at run start
- native `commit_and_push_to_github` execution without ACP
- native `build_archive_and_push_connect` execution in sandbox/staging safe mode
- structured release failure/success receipts and strict lineage-gated terminal backfill
- canonical release artifact-path persistence for workflow transition truth
- northbound readback for frozen delivery config and release evidence
- protected-branch rejection (`main` / `master`)

Command:

```bash
./scripts/test-gate.sh proposal-045
```

### `proposal-047|p047`

Control-plane Rust workspace verification gate.

Scope:

- `control-plane` Rust workspace (`cargo test --workspace`)

Use when:

- validating the control-plane workspace test suite on the current head
- reproving the proposal-047 control-plane slice without pulling in unrelated app or UI gates

Host policy:

- local Rust toolchain required; no iOS/macOS simulator needed
- executes in-process against the `control-plane/` workspace

Command:

```bash
./scripts/test-gate.sh proposal-047
```

Important:

- this gate is the canonical proof path for the proposal-047 control-plane workspace slice
- the runner also accepts the `p047` alias for parity with other proposal gates

### `proposal-029-mcp|p029-mcp`

MCP + GraphQL northbound auth, capability filtering, and audit-journaling gate for the Rust control-plane.

Scope:

- principal-table bootstrap (owner-only `0o600` file mode on Unix, one-time token log, fail-closed on empty table)
- bearer auth on MCP HTTP (`POST /mcp`), MCP stdio (`initialize.params.clientInfo.principal_token`), GraphQL HTTP (`POST /graphql`), and GraphQL WebSocket (`/graphql/ws` via `on_connection_init`)
- per-class capability filtering for MCP `tools/list`, `tools/call`, `resources/list`, `resources/read`, including the Steward trio policy (`steward.run_analysis` operator-only, `steward.list_analyses` + `steward.get_analysis` operator/observer, agent excluded)
- GraphQL mutation class policy: UI/default principals may execute only `approveApproval` and `rejectApproval`; non-approval command mutations are MCP-only
- `command_journal` caller metadata (`caller_surface`, `caller_principal_id`, `caller_principal_class`, `caller_tool`) populated per MCP command tool and per GraphQL mutation
- the §8.1 redaction matrix (one test per `Command` variant decision)
- `journal_id` surfacing inside `content[0].text` on MCP command tools and as `journalId: ID!` on approval GraphQL mutation payload wrappers
- typed `DeliveryPreflight` object on MCP `runs.start` blocked preflight responses
- command write-path boundary: non-approval operator commands route through MCP, while GraphQL remains read/subscription plus approval-only mutation surface
- dogfood `.mcp.json` / `CLAUDE.md` consistency (repo-root `.mcp.json` registers `chainworks-control-plane` with `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`; `CLAUDE.md` documents the env var and URL)
- full `cargo test --workspace` regression after the focused inventory

Host policy:

- local Rust toolchain required; no UI host or simulator needed

Command:

```bash
./scripts/test-gate.sh proposal-029-mcp
```

Important:

- this gate is the canonical proof path for [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md)
- the runner also accepts the `p029-mcp` alias
- the gate is distinct from `proposal-029|p029` (second-wave ACP runtime profiles); the two labels coexist
- the gate enumerates a fixed focused inventory and grep-checks each `cargo test` output for a matching `test <name>` line, so rename/typo/delete fails the gate independently of the test body
- `cargo test --workspace` is not a substitute; the enumerated inventory is the contract

### `proposal-042|p042`

Implemented local daemon lifecycle, supervision, health/readiness, packaging, SQLite startup safety, failed-serve, diagnostics, and request-correlation gate.

The original proposal document has been retired after implementation. The gate name remains `proposal-042` because the Rust/Swift focused inventories, saved proof logs, and downstream prerequisite checks use that identifier.

Scope:

- `domain::lifecycle` status types, degraded/failure disjointness, and failure invariant tests
- lifecycle reporter transitions and `DaemonStatusChanged` broadcasts
- `/health` and `/ready` status-code matrix
- GraphQL `daemonStatus` and `daemonStatusChanged` query/subscription auth and shape
- packaged-mode PID lock, crash-budget helpers, loopback enforcement, `daemon.port`, cwd, and build-SHA behavior
- SQLite migration preflight, backup, no-downgrade safety, and migration-error mapping
- failed-serve status readback, GraphQL auth, and MCP JSON-RPC error envelopes
- log redaction, log retention, request-id middleware, MCP request-context propagation, and command-journal correlation
- Swift daemon lifecycle client, diagnostics bundle, packaged binary checks, supervisor behavior, and crash-budget reset tests
- full Rust workspace regression

Host policy:

- local macOS/Xcode and Rust toolchain required
- no release-host Developer ID credentials required
- no live provider account, simulator, or network required

Command:

```bash
./scripts/test-gate.sh proposal-042
```

Important:

- `p042` is accepted as an alias
- this is the canonical implementation-ready proof path for [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md)
- the saved green evidence log is `docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log`
- release packaging readiness is a separate release-host lane, not part of normal implementation readiness

### `proposal-042-packaging|p042-packaging`

Release-host packaging proof for the local daemon embedded in Chainworks Forge.app.

Scope:

- Release archive/export
- embedded `Contents/MacOS/chainworks-forge-daemon` presence and executable bit
- strict codesign verification
- matching Developer ID Application authority between the app bundle and embedded daemon
- expected Team ID allow-list check
- notarization staple validation
- Gatekeeper assessment
- packaged app launch-to-Ready proof through `daemon.port` and `/health`

Host policy:

- release host only
- requires `scripts/packaging.env` populated from `scripts/packaging.env.example`
- requires Developer ID and notarization credentials
- fails closed with "not on a release host" on normal developer workstations

Command:

```bash
./scripts/test-gate.sh proposal-042-packaging
```

Important:

- every successful run writes `docs/evidence/042-local-daemon-lifecycle/release-gate-YYYYMMDDTHHMMSSZ.log`
- shipping a signed/notarized packaged app requires this release-host evidence

### `proposal-048|p048`

Failed-stage evidence, delivery preflight, and MCP resolution gate for the Rust control-plane slice.

Scope:

- durable P048 DB/domain persistence round-trip
- delivery-preflight passing and blocked-start behavior
- GraphQL delivery-preflight readback
- MCP `runs.get` and `run://{run_id}` delivery-preflight readback
- ACP `session/new.mcpServers` serialization
- engine fail-closed MCP resolution persistence, including explicit empty actual truth and blocked-before-session observation
- failed-stage evidence packet V1 shape
- failed-stage evidence `reports.get` and `report://{run_id}` readback
- typed GraphQL blocked preflight payload
- GraphQL stage `executions` MCP truth parity
- MCP `reports.get` execution-level MCP truth
- MCP `report://{run_id}` execution-level MCP truth

Use when:

- changing delivery preflight, failed-stage evidence, MCP resolution, ACP MCP payloads, GraphQL blocked-start shape, or MCP report/resource readback
- reproving the implemented failed-stage evidence, delivery-preflight, and MCP-resolution control-plane slice

Host policy:

- local Rust toolchain required; no iOS/macOS simulator needed
### `proposal-049|p049`

Steward analysis system gate for the Rust control-plane slice.

Scope:

- workflow metadata and parsed snapshot freezing
- run-start frozen cohort/project/snapshot persistence
- Steward config validation, non-empty threshold ownership, default fallback, parsed catalog hashing, and pending config-change bootstrap
- P049-shaped `steward_analyses`, run-link, recommendation, failed-analysis, and work-item persistence
- cohort classification, explicit primary cohort grouping, legacy-pre-P049 exclusion, deterministic metrics/dossiers, anomaly signals, and recommendation persistence
- active-catalog Steward IO paths under `CHAINWORKS_META_ROOT`, including production `StewardAnalysis` work-item execution through ACP-backed `system_steward` and `steward_auditor` lanes
- manual, post-run interval, and config-change trigger convergence on `WorkItemKind::StewardAnalysis`
- run-owned drift detection persistence from startup recovery
- GraphQL, MCP tool, and `steward-analysis://` readback surfaces

Host policy:

- local Rust toolchain required; no UI host or simulator needed
- executes in-process against the `control-plane/` workspace

Command:
```bash
./scripts/test-gate.sh proposal-048
./scripts/test-gate.sh proposal-049
```

### `proposal-077|p077`

Bounded implementation closeout readiness gate — Phase-1 Rust slice.

Scope (what this gate actually covers):

- `implementation_closeout_readiness_v1` decision-matrix validation (Rust domain/db/engine)
- proposal gate domain contracts and status normalization
- audit verdict policy enforcement (synthesizer unit tests)
- bounded code/refine loop and soft convergence checkpoints (memory-level)
- closeout fingerprinting and latency budget validation
- DB closeout transaction atomicity (db integration tests)
- accessor routing proof (in-memory, not against live orchestrator graph)

NOT covered by this gate (require additional integration gates or manual evidence):

- integrated orchestrator transition guard against live SQLite (state_9 real run)
- GraphQL/MCP runs.get/list closeout-readiness readback parity tests
- macOS UI/accessibility surfaces and VoiceOver fixtures
- Swift workspace tests

Use when:

- changing implementation closeout readiness logic, status normalization, or decision rules
- changing proposal gate domain contracts or DB closeout transaction
- validating bounded loop convergence or soft checkpoint behavior at the synthesizer level
- reproving the Proposal 077 Phase-1 Rust domain/db/engine slice

Host policy:

- local Rust target only; no Swift or UI build required

Command:

```bash
./scripts/test-gate.sh proposal-077
```

Important:

- this gate covers the Phase-1 Rust domain/db/engine slice of Proposal 077; full R14 acceptance requires additional integrated transition, GraphQL/MCP parity, and macOS fixture coverage
- it validates that a run cannot enter manual release without a resolved proposal gate or with pending code blockers
- it verifies the transition evaluation reads the active `implementation_closeout_readiness_v1` contract truth

### `p051-scaffold`

Historical bridge-pool scaffold gate alias for the shared Xcode MCP bridge pool substrate.

Scope:

- static stale-guidance check for the stable Xcode MCP bridge pool reference
- workflow/catalog Xcode broker field and lint fixtures
- DB/domain Xcode runtime observation append and persistence fixtures
- ACP provider capability / brokered Xcode intent fixtures
- Xcode MCP bridge pool lease, capacity, target-resolution, and observation fixtures
- engine observation-sink persistence fixture
- GraphQL and MCP server compile checks for the observation/readback contract

Use when:

- changing brokered Xcode MCP intent resolution, provider HTTP MCP capability checks, or session/new lease attachment
- changing Xcode runtime observation shape, append semantics, or broker failure classes
- changing daemon/API scaffolding that must remain compatible with the full bridge-pool gate
- reproving scaffold readiness before broader live Xcode proof

Host policy:

- local Rust toolchain required
- no live provider account, Xcode consent interaction, simulator, remote UI host, or live dogfood run required
- fixture evidence only; this gate does not prove production packaged-daemon release readiness

Command:

```bash
./scripts/test-gate.sh p051-scaffold
```

Important:

- the gate fails if [xcode-mcp-bridge-pool.md](xcode-mcp-bridge-pool.md) reintroduces stale contrary guidance for the implemented contract
- bridge-pool dependency/readiness evidence lives under [../evidence/051-shared-xcode-mcp-bridge-pool/](../evidence/051-shared-xcode-mcp-bridge-pool/)

### `proposal-051|p051`

Historical gate aliases for the shared Xcode MCP bridge pool fixture/readback gate.

Scope:

- all `p051-scaffold` checks
- domain artifact-contract compatibility fixture used by bridge-pool readback
- repeated workflow/DB/ACP/engine bridge-pool fixture inventory under the full gate target directory
- GraphQL and MCP server compile checks
- focused Swift readback tests for timeline inspector and daemon lifecycle broker health consumption

Use when:

- changing implemented shared Xcode MCP bridge pool behavior
- changing Xcode runtime observation readback in GraphQL, MCP reports, or Swift operator surfaces
- preparing fixture/readback evidence for the implemented bridge-pool contract

Host policy:

- local Rust and macOS Swift test toolchains required
- no remote UI host or app-launched dogfood proof is run by this gate
- this is a fixture/readback gate, not production packaged-daemon release proof

Command:

```bash
./scripts/test-gate.sh proposal-051
./scripts/test-gate.sh p051
```

Important:

- `proposal-051` and `p051` are aliases
- the stable behavior reference is [xcode-mcp-bridge-pool.md](xcode-mcp-bridge-pool.md)
- scoped broker/readback closeout sign-off is recorded in [../evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md](../evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md)
- broad release or broad `shim_enforced` rollout still requires the P042 `proposal-042-packaging` release-host proof

### `proposal-054|p054`

Implementation completeness and handoff contract gate.

Scope:

- `implementation_self_assessment_v2` schema and validation
- v2-over-v1 precedence and migration truth
- workflow transitions for `complete`, `handoff_required`, `blocked`, and `needs_code_fixes`
- `approval.rejected == true` loopback from state_6 to state_5
- `ReleaseGateView` verification-truth resolution from v2
- nullable summary fields in GraphQL and MCP

Use when:

- changing implementation assessment schema, status normalization, or transition logic
- changing implementation-approval rejection behavior
- changing release-gate verification truth sourcing

Command:

```bash
./scripts/test-gate.sh proposal-054
```

Important:

- this gate is the canonical proof path for the implemented [implementation self-assessment and handoff contract](output-contracts-failure-evidence-and-recovery.md#implementation-self-assessment-and-handoff); the historical `proposal-054|p054` alias is retained for reproducibility
- it validates that rejected implementation approvals loop back to proposal refinement without manual repair

### `proposal-050|p050`

Per-run workspace isolation gate for the Rust control-plane.

Scope:

- per-run `chainworks_meta_root` derivation at run creation
- `resolve_path_template` uses run meta-root override, does not consult process env for `CHAINWORKS_META_ROOT`
- `exists()` and `artifact.field` transition checks resolve against per-run meta root; post-P050 runs do NOT fall back to shared `artifact_root`
- `normalize_path_for_worktree` exempts meta-root paths from worktree rewrite
- `normalize_artifacts` source-side isolation: post-P050 runs search only `artifact_root/{run_id}`
- ACP adapter env handoff: all five adapters inject `CHAINWORKS_META_ROOT`
- GraphQL and MCP read-only meta-root readback
- legacy NULL-meta-root backward compatibility

Host policy:

- local Rust toolchain required; no UI host or simulator needed

Command:

```bash
./scripts/test-gate.sh proposal-050
```

Important:

- `proposal-050` is the canonical proof path for [per-run-workspace-isolation.md](per-run-workspace-isolation.md)
- the runner also accepts the `p050` alias
- this gate runs `cargo test --workspace` from `control-plane/`

### `proposal-053|p053`

Bounded ACP artifact discovery and startup latency gate.

Scope:

- Phase 0 cap-validation and Phase 1 security evidence artifacts under `docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/`
- full Phase 0 cap-validation schema, including sampled execution IDs, readiness timing, envelope/aggregate cap selections, and discovery ownership
- Phase 1 evidence pack:
  - `manual-latency-spot-check.md`, including the manual reference-workspace measurement and observed `acp_pre_initialize_local_latency_ms`
  - `operator-clarity-evidence.md`
  - `phase-1-retrospective.md`
- `control-plane` discovery logic (`crates/domain/src/discovery.rs`)
- engine discovery settlement pipeline
- `OutputDiscoveryDecision` and `agent_execution_discovery_diagnostics` persistence
- pre-prompt metadata capture and digest-backed validation
- bounded meta-root discovery (max 500 files, 10 MiB aggregate)
- discovery filesystem operation-recorder coverage for bounded traversal and metadata reads
- trait-backed `DiscoveryFilesystem` fake coverage for deterministic bounded-discovery filesystem tests
- stale-vs-absent required-output settlement and GraphQL/MCP stale-count readback
- legacy broad discovery opt-in policy
- GraphQL and MCP readback for discovery diagnostics

Use when:

- changing artifact discovery, inference, or settlement logic
- changing ACP pre-prompt metadata capture
- changing meta-root bounding or legacy discovery policy

Host policy:

- local Rust toolchain required
- local target only; no UI host or simulator needed

Command:

```bash
./scripts/test-gate.sh proposal-053
```

Important:

- `p053` is accepted as an alias
- broad recursive discovery is disabled by default
- discovery settlement is governed by the engine-owned pipeline, not provider-side heuristics
- a `gate_only_internal` cap-validation artifact is sufficient for same-tree control-plane validation, but production exposure requires a refreshed production sampling/signoff artifact
- the gate enforces the full declared Phase 0 cap-validation schema rather than a reduced subset
- the Phase 1 evidence pack must exist in the stable evidence directory before release signoff

### `proposal-057|p057`

Historical proposal gate for the implemented canonical artifact contracts and run-state
projection contract. The proposal document has been retired after implementation; the
gate name remains stable because tests, scripts, and historical proof records use that
identifier.

Scope:

- typed artifact contract status normalization for machine-consumed reports
- active-index SQLite owner and exported `active-index.json` projection semantics
- generated run-state projection from DB truth plus active contracts
- degraded output policy default-deny / explicit-allow contract
- typed operator overrides with capability-gated MCP ownership
- GraphQL/MCP readback parity for canonical artifact statuses, override truth, and projection warnings

Use when:

- changing transition evaluation for artifact status fields
- changing artifact import, supersession, active contract pointers, or run-state projection
- changing canonical artifact override command/readback behavior
- changing degraded partial-output settlement policy

Host policy:

- local Rust toolchain required
- control-plane-only Rust evidence; this gate must not invoke Xcode or Swift test plans
- no simulator, daemon process, UI target, or network required

Command:

```bash
./scripts/test-gate.sh proposal-057
```

Prerequisite posture:

- P037 control-plane evidence bucket: P057 consumes the failed/partial ACP execution settlement prerequisite through its own Rust engine degraded-output tests instead of invoking the broader Xcode-backed `proposal-037` gate.
- Same-tree composed gates: `proposal-043` and `proposal-050` run before P057-local assertions.
- P057 prerequisite: the implementation-completeness handoff contract is now stable reference truth in [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md#implementation-self-assessment-and-handoff), and `proposal-054|p054` is retained as the reproducible gate alias for that implemented contract. If P057 changes implementation self-assessment or handoff transition semantics, compose the retained `proposal-054` gate or document the accepted schema delta in this gate reference.
- P057 prerequisite waiver: P056, dated 2026-04-19. No registered `proposal-056` gate exists on this tree. The canonical artifact-contract implementation keeps new artifact contract and override payloads in typed domain/db/engine modules and proves that slice locally; compose `proposal-056` here when it is registered. Rollback/hold rule: if P056 registers a module-boundary gate that changes artifact contract ownership, pause canonical artifact-contract closure until the P056 gate is same-tree green or this waiver names the accepted boundary delta.

Important:

- `p057` is accepted as an alias
- the active artifact index is canonical in SQLite; `active-index.json` is only an exported projection and stale/partial exports must never drive transition truth
- degraded partial-output settlement is denied by default and requires explicit compiled stage policy before `valid_outputs_from_failed_execution` can satisfy transitions
- typed operator overrides are separate from raw report files, require operator capability, write command journal evidence, expire at `expires_at_stage`, and remain visible in readback after expiry
- the gate fails closed if canonical status normalization, active-index SQLite ownership, stale export rebuild, raw artifact fallback denial, degraded policy, typed overrides, or GraphQL/MCP readback parity evidence is missing

### `proposal-058|p058`

Implemented regression gate for ACP provider failure classification and session artifact ownership.

The original proposal document has been retired after implementation. The gate name
remains `proposal-058` because the Rust test targets and historical proof lane use that
identifier.

Scope:

- typed `AgentFailureKind`, `AgentOutputSettlement`, runtime facts, and operator action hints
- typed ACP/P037/P045/P051/P057 failure-observation classification matrix coverage
- runtime failure redaction fixtures and P045 recovery-action mapping from P058 runtime facts
- durable `agent_execution_runtime_facts` read/write behavior
- artifact source-generation claims, including `superseded_pending_retry`
- `InvokeAgent` claim/start ownership: generic work-queue claim skips `InvokeAgent`, while the engine-owned start transaction pre-creates exactly one `agent_executions` row and matching source-generation claim
- retry enqueue-to-claim late-output rejection and source-generation CAS behavior
- GraphQL/MCP runtime-facts parity and artifact source provenance
- no-secret redacted runtime failure readback

Use when:

- changing ACP provider/transport error classification
- changing executor output validation settlement or degraded output behavior
- changing session reuse, retry supersession, or late-output handling
- changing artifact active-index source provenance
- changing GraphQL or MCP execution truth readback

Host policy:

- local Rust toolchain required
- no live provider account, Xcode, simulator, daemon process, UI target, network, or real quota exhaustion required

Command:

```bash
./scripts/test-gate.sh proposal-058
```

Prerequisite posture:

- Same-tree dependency evidence: P058 consumes P037 timeout semantics, P045 recovery/retry semantics, P051 Xcode MCP observations, and canonical artifact contracts.
- The focused P058 gate uses fixture/fake transport coverage for those consumed seams rather than requiring live provider, live Xcode, or UI evidence.

Important:

- `p058` is accepted as an alias
- runtime facts are durable typed execution truth, not log parsing
- `InvokeAgent` provider startup must use the pre-created execution identity from the claim/start DTO; creating a second execution row after the claim boundary is a gate failure
- P058 claim/start tests must run; compiling them with `--no-run` is not sufficient proof
- DB claim-start and MCP parity are executed single-job in gate-owned target directories so stale shared `target/` artifacts cannot satisfy or block the proof
- `ignored_late_outputs` is output settlement truth, not an `AgentFailureKind`
- stale output from `closed`, `superseded`, or `superseded_pending_retry` claims must never update active artifact truth
- the gate fails closed if runtime facts, source-generation claims, pending retry supersession, artifact provenance, or GraphQL/MCP runtime-facts parity evidence is missing

### `proposal-065|p065`

Operator retry instruction contract gate.

Scope:

- `stages.retry` MCP input schema extension with `operator_instruction`
- `RetryStageCmd` domain command extension
- validation for 1-2000 character length and control character rejection
- durable command-journal persistence and redaction
- retry-attempt parent binding and child delivery persistence
- targeted retry work-item payload injection
- full-stage retry fan-out delivery logic
- operator-only capability enforcement

Use when:

- changing retry instruction validation or persistence
- changing orchestrator/executor instruction delivery
- changing readback/provenance for operator instructions

Host policy:

- local Rust toolchain required; no UI target or simulator needed

Command:
```bash
./scripts/test-gate.sh proposal-065
```

### `proposal-060|p060`

Deterministic reviewer routing and expanded reviewer catalog gate.

Scope:

- `proposal_review_router` SystemTask and `executor_mode: system.routing`
- `AgentSelectionPlanV1` and `RoutingReceipt` persistence
- deterministic scoring formula parity (Swift/Rust)
- mandatory reviewer rules, force_include/exclude overrides, and under_specified fallback
- dynamic reviewer materialization via `dynamic_parallel`
- `selected_outputs_from` aggregation for dynamic reviewer sets
- core specialist reviewer enablement (macOS, Apple architecture, Rust architecture, reliability, security, API contract, observability/rollout)
- `operator_debug_routing_evidence` capability and hash-only vs raw evidence projection
- Phase 0b control artifacts: `fixed-quartet-inventory`, `frozen-snapshot-helper-inventory`, `implementation-ticket-map`, `proposal-review-baseline`, `routing-calibration-report`, `routing-contract-fixtures`, and `storage-compatibility-matrix`

Use when:

- changing reviewer routing logic, scoring, or selection rules
- changing `dynamic_parallel` materialization or `selected_outputs_from` aggregation
- adding or enabling new specialist reviewers in the catalog
- changing routing-evidence projection or redaction policy
- updating routing fixtures or control artifacts

Host policy:

- local Rust and macOS Swift test toolchains required
- no live provider account, simulator, or network required
- verifies Phase 0b control artifacts at their canonical paths before runtime work proceeds

Command:

```bash
./scripts/test-gate.sh proposal-060
```

Important:

- `p060` is accepted as an alias
- the gate fails closed if any Phase 0b control artifact is missing, stale, or revision-mismatched
- same-head parity for plan_hash and evidence IDs is enforced across Swift and Rust

### `full`

Expensive repo-wide sign-off gate.

Scope:

- source guardrails
- app build
- full `xcodebuild test`

Use when:

- preparing proposal sign-off
- validating the repository baseline before merge or release work

Important:

- `full` is still a repository-baseline gate, not a substitute for proposal-specific app-launched dogfood proof
- for repo-backed delivery sign-off, agents may need both `full` and the app-launched evidence flow described in [agent-ui-test-execution.md](agent-ui-test-execution.md)
- because `full` includes the UI target, the gate runner also treats it as remote-only

Command:

```bash
./scripts/test-gate.sh full
```

Canonical remote form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh full"
```

## Recommended Usage

### Normal implementation loop

```bash
./scripts/test-gate.sh fast
```

### UI-heavy work

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"
```

### Provider-platform work

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-006"
```

### Proposal-loop fidelity work

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-022"
```

### Artifact rendering proof

```bash
./scripts/test-gate.sh proposal-027
```

### ACP-only runtime proof
```bash
./scripts/test-gate.sh proposal-033
```

### Scheduler and backpressure proof
```bash
./scripts/test-gate.sh proposal-061
```

### Before sign-off

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh full"
```

### UI quality proof

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-012"
```

### Context-strategy proof

```bash
./scripts/test-gate.sh proposal-019
```

## Why This Exists

The repository has a real mix of:

- cheap domain/runtime tests
- medium-cost provider/platform slices
- expensive macOS UI automation
- broad baseline tests that catch unrelated crashes outside the active proposal

Running them all on every edit burns time and makes failures harder to interpret. These gates make the cost and purpose of each layer explicit.

## Related Docs

- [test-suite-architecture.md](test-suite-architecture.md)
- [agent-ui-test-execution.md](agent-ui-test-execution.md)
- [provider-platform.md](provider-platform.md)
- [operator-experience.md](operator-experience.md)

### `proposal-057|p057`

Historical proposal gate for the implemented canonical artifact contracts and run-state
projection contract. The proposal document has been retired after implementation; the
gate name remains stable because tests, scripts, and historical proof records use that
identifier.

Scope:

- typed artifact contract status normalization for machine-consumed reports
- active-index SQLite owner and exported `active-index.json` projection semantics
- generated run-state projection from DB truth plus active contracts
- degraded output policy default-deny / explicit-allow contract
- typed operator overrides with capability-gated MCP ownership
- GraphQL/MCP readback parity for canonical artifact statuses, override truth, and projection warnings

Use when:

- changing transition evaluation for artifact status fields
- changing artifact import, supersession, active contract pointers, or run-state projection
- changing canonical artifact override command/readback behavior
- changing degraded partial-output settlement policy

Host policy:

- local Rust toolchain required
- control-plane-only Rust evidence; this gate must not invoke Xcode or Swift test plans
- no simulator, daemon process, UI target, or network required

Command:

```bash
./scripts/test-gate.sh proposal-057
```

Prerequisite posture:

- P037 control-plane evidence bucket: P057 consumes the failed/partial ACP execution settlement prerequisite through its own Rust engine degraded-output tests instead of invoking the broader Xcode-backed `proposal-037` gate.
- Same-tree composed gates: `proposal-043` and `proposal-050` run before P057-local assertions.
- P057 prerequisite: the implementation-completeness handoff contract is now stable reference truth in [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md#implementation-self-assessment-and-handoff), and `proposal-054|p054` is retained as the reproducible gate alias for that implemented contract. If P057 changes implementation self-assessment or handoff transition semantics, compose the retained `proposal-054` gate or document the accepted schema delta in this gate reference.
- P057 prerequisite waiver: P056, dated 2026-04-19. No registered `proposal-056` gate exists on this tree. The canonical artifact-contract implementation keeps new artifact contract and override payloads in typed domain/db/engine modules and proves that slice locally; compose `proposal-056` here when it is registered. Rollback/hold rule: if P056 registers a module-boundary gate that changes artifact contract ownership, pause canonical artifact-contract closure until the P056 gate is same-tree green or this waiver names the accepted boundary delta.

Important:

- `p057` is accepted as an alias
- the active artifact index is canonical in SQLite; `active-index.json` is only an exported projection and stale/partial exports must never drive transition truth
- degraded partial-output settlement is denied by default and requires explicit compiled stage policy before `valid_outputs_from_failed_execution` can satisfy transitions
- typed operator overrides are separate from raw report files, require operator capability, write command journal evidence, expire at `expires_at_stage`, and remain visible in readback after expiry
- the gate fails closed if canonical status normalization, active-index SQLite ownership, stale export rebuild, raw artifact fallback denial, degraded policy, typed overrides, or GraphQL/MCP readback parity evidence is missing

### `proposal-058|p058`

Implemented regression gate for ACP provider failure classification and session artifact ownership.

The original proposal document has been retired after implementation. The gate name
remains `proposal-058` because the Rust test targets and historical proof lane use that
identifier.

Scope:

- typed `AgentFailureKind`, `AgentOutputSettlement`, runtime facts, and operator action hints
- typed ACP/P037/P045/P051/P057 failure-observation classification matrix coverage
- runtime failure redaction fixtures and P045 recovery-action mapping from P058 runtime facts
- durable `agent_execution_runtime_facts` read/write behavior
- artifact source-generation claims, including `superseded_pending_retry`
- `InvokeAgent` claim/start ownership: generic work-queue claim skips `InvokeAgent`, while the engine-owned start transaction pre-creates exactly one `agent_executions` row and matching source-generation claim
- retry enqueue-to-claim late-output rejection and source-generation CAS behavior
- GraphQL/MCP runtime-facts parity and artifact source provenance
- no-secret redacted runtime failure readback

Use when:

- changing ACP provider/transport error classification
- changing executor output validation settlement or degraded output behavior
- changing session reuse, retry supersession, or late-output handling
- changing artifact active-index source provenance
- changing GraphQL or MCP execution truth readback

Host policy:

- local Rust toolchain required
- no live provider account, Xcode, simulator, daemon process, UI target, network, or real quota exhaustion required

Command:

```bash
./scripts/test-gate.sh proposal-058
```

Prerequisite posture:

- Same-tree dependency evidence: P058 consumes P037 timeout semantics, P045 recovery/retry semantics, P051 Xcode MCP observations, and canonical artifact contracts.
- The focused P058 gate uses fixture/fake transport coverage for those consumed seams rather than requiring live provider, live Xcode, or UI evidence.

Important:

- `p058` is accepted as an alias
- runtime facts are durable typed execution truth, not log parsing
- `InvokeAgent` provider startup must use the pre-created execution identity from the claim/start DTO; creating a second execution row after the claim boundary is a gate failure
- P058 claim/start tests must run; compiling them with `--no-run` is not sufficient proof
- DB claim-start and MCP parity are executed single-job in gate-owned target directories so stale shared `target/` artifacts cannot satisfy or block the proof
- `ignored_late_outputs` is output settlement truth, not an `AgentFailureKind`
- stale output from `closed`, `superseded`, or `superseded_pending_retry` claims must never update active artifact truth
- the gate fails closed if runtime facts, source-generation claims, pending retry supersession, artifact provenance, or GraphQL/MCP runtime-facts parity evidence is missing

### `proposal-061|p061`

SQLite write serialization and executor backpressure gate.

The `proposal-061|p061` names are retained historical aliases for the implemented
SQLite write-serialization, scheduler-backpressure, host-interruption, and
generated-state housekeeping contract documented in
[`rust-control-plane.md`](rust-control-plane.md).

Scope:

- `InvokeAgentCapacityConfig` defaults and provider alias normalization
- Capacity accounting for global, provider, and per-run caps
- Capacity-aware claim/start leaves blocked work pending and does not create agent_executions
- Fair scheduler selection via `scheduler_service_state` durable least-recently-served state
- Hot-index-backed pending InvokeAgent scans and active-count queries with EXPLAIN/query-plan assertions
- ApproveStage, RetryStage, and CancelRun p95 command latency below 2 seconds under 20 active fake agents
- Retry/Startup-repair transaction boundaries, atomic supersession, and requeue through capacity gates
- Projection freshness, zero-count cleanup, all-blocked scan updates, and stale readback markers for scheduler summaries
- GraphQL and MCP parity for `schedulerHealthSummary` and queue summaries
- Sustained-backpressure subscription/MCP notification fire and clear behavior
- Simulated host sleep/wake and network migration classification, process cleanup, jittered retry under caps, and quota exemption
- DB contention instrumentation in runtime health logs and projections
- Generated-state housekeeping safety for active/blocked run outputs, managed worktree targets, source files, run artifacts, SQLite database files, stale ACP homes, and unmanaged worktrees

Use when:

- changing scheduler capacity, fairness, or backpressure logic
- changing SQLite transaction boundaries or write coordination for operator commands
- changing host-interruption detection, classification, or recovery
- changing scheduler-health or queue-summary readback surfaces

Host policy:

- local Rust toolchain required
- no live provider account, Xcode, simulator, daemon process, UI target, or network required

Command:

```bash
./scripts/test-gate.sh proposal-061
```

Important:

- `p061` is accepted as an alias
- the gate asserts p95 command latency under load; ensure the local host is not under extreme unrelated CPU pressure
- query-plan assertions prove that scheduler scans do not regress to full table scans at fixture scale
- host-interruption retries must be exempt from provider quota retry budget
- the gate fails closed if capacity gates, fair selection, p95 latency, atomic supersession, projection parity, backpressure notifications, or host-interruption classification evidence is missing

### `proposal-064|p064`

P064 Phase 0 main-sync and cross-run knowledge readback contract gate.

Scope:

- P064 Phase 0 dogfood baseline artifact and kickoff record are present and schema-versioned
- migration `033_p064_main_sync_and_knowledge_capsules.sql` freezes main-sync, barrier, knowledge-capsule, work-item, and background-lease storage contracts
- `domain::main_sync` enum/value contracts round-trip
- MCP main-sync and knowledge-capsule tools remain registered as capability ids but hidden while runtime modes are off
- GraphQL exposes projection-backed JSON readback for main-sync status, accepted/pending command state, barriers, active consumers, and knowledge-capsule attachments

Command:

```bash
./scripts/test-gate.sh proposal-064
```

Important:

- `p064` is accepted as an alias
- this is a Phase 0 contract/readback gate, not proof that Git mutation or capsule prompt injection is enabled
- later P064 phases must extend this gate before shipping repositories, sync execution, dirty preservation, conflict routing, or prompt injection
