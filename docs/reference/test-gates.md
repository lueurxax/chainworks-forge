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

### Session Observability GraphQL

Session observability GraphQL proof gate. The `proposal-046` and `p046` command names are retained historical aliases for reproducible verification. Operational truth lives in [rust-control-plane.md](rust-control-plane.md#graphql) and [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md#operator-surfaces).

Scope:

- runs `cargo test -p graphql-server --test proposal_046_session_graphql` plus the `p046_retry_db` unit-test lane in `graphql-server` for the pinned retry policy; these test names are retained historical aliases
- verifies the operator readback fixture under `docs/evidence/rollout-contract/operator-readback/`, the session observability negative fixtures under `docs/evidence/rollout-contract/negative/`, and the retained historical alias P046 metric inventory in `graphql-server` source
- covers SDL snapshots (Connection/Edge/PageInfo shape, cursor nullability, absence of `resetSession` and any equivalent session-control mutation), explicit disabled-schema behavior, operator-read authorization across `runId`- and ID-based resolvers, parent-run/lineage ownership lookup with not-found-or-not-visible behavior, cursor pagination and `limit+1` `hasNextPage` semantics, sanitized invalid-cursor errors, derived non-secret replacements for `providerSessionId`/`bindingFingerprint`/`invocationOwnerKey`/`workingDirectory`, the pinned transient sqlite retry policy (3 attempts, 50/150 ms backoff, <=250 ms sleep budget, >=250 ms resolver headroom, deterministic `db_unavailable` or `UNKNOWN/transient_db_unavailable` on exhaustion), event-details redaction (`p046_event_details_redaction_v1` allowlist, unknown-shape fail-closed), and `sessionStatusChanged` run-filtering, per-emission authorization recheck, 64-payload per-subscriber buffer, at-most-once `resyncRequired`, and slow-consumer disconnect; `p046_*` labels are retained historical aliases

Host policy:

- local target only; runtime/unit proof lane without a UI target

Command:

```bash
./scripts/test-gate.sh proposal-046  # retained historical alias
./scripts/test-gate.sh p046          # retained historical alias
```

Important:

- session observability is read/subscription-only by contract: no GraphQL `resetSession`, `closeSession`, `invalidateSession`, or equivalent session-control mutation may appear in the schema; reset remains MCP-only
- session observability GraphQL fields are enabled by default; clients still must gate documents on capability/schema discovery so explicit disabled-schema mode does not produce validation errors
- no SQL schema migration or additive index is allowed in this slice; profiling-driven indexing requires a follow-up proposal revision

### `proposal-036|p036`

macOS operator navigation and read-model UX gate. The alias retains the historical proposal number; the stable behavior contract is [macos-operator-navigation.md](macos-operator-navigation.md).

Scope:

- `Proposal036UXConsolidationTests` — navigation shell, route compatibility, presenter, Definitions, and timeline batching coverage
- `Proposal031ThinGraphQLReadBoundaryTests` — thin GraphQL read boundary parity after Runs workbench presenter consolidation
- `RunTimelineInspectorViewTests` — timeline presentation model, batching, Reduce Motion, and summary-card behavior
- UI smoke cases `testProposal036NavigationShellParity` and `testProposal036DefinitionsSegmentedWrapper` (remote-only by repo policy)

Use when:

- changing the four-tab navigation shell, route compatibility map, or `CHAINWORKS_UI_TEST_INITIAL_TAB` handling
- changing `RunsWorkbenchPresentationModel`, P036 deferred-state types, or Runs/Definitions/Settings layout ownership
- changing timeline batching, Reduce Motion behavior, or agent completion summary cards

Host policy:

- on local hosts, `proposal-036` runs the build plus unit/runtime slices and skips the UI smoke slice
- on approved remote UI hosts, the same `proposal-036` gate also runs the UI smoke cases
- UI smoke tests are remote-only per repository policy and must run via the approved host (`test@SMacBook.local`)

Command:

```bash
./scripts/test-gate.sh proposal-036
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-036"
```

Important:

- this gate is the repo-owned proof lane for the consolidated macOS operator shell
- inline approval rendering follows the [thin-client read-model affordance contract](thin-client-read-model-affordance-contract.md); legacy Approvals routes redirect into Runs with waiting-approval focus after the Phase 2c top-level tab removal

### Retained historical alias: `proposal-093|p093`

Retained historical alias for the live agent Timeline UX and readability gate. The stable behavior is documented in [macOS Operator Navigation and Read-Model UX](macos-operator-navigation.md#timeline); this section owns only the proof lane.

Scope:

- focused `Proposal036UXConsolidationTests` slices for chunk accumulation, terminal response summaries, truthful raw-detail availability, newest-first ordering, agent selector presence, formatting, and expandable card behavior
- `Proposal031ThinGraphQLReadBoundaryTests` for thin read-model compatibility
- `graphql-server` runtime timeline tests for additive daemon-owned timeline fields and the raw-detail resolver surface

Use when:

- changing live Timeline cards, response chunk coalescing, newest-first ordering, active-agent selection, raw-detail copy labeling, or formatted expanded details
- changing `runtimeStatusChanged` fields or the `timelineRawDetail(handle:)` resolver contract

Host policy:

- on local hosts, the retained historical alias runs the build plus non-UI focused tests and skips remote UI proof
- on approved remote UI hosts, the same gate may include UI smoke coverage
- UI smoke tests remain remote-only per repository policy

Command:

```bash
./scripts/test-gate.sh proposal-093 # retained historical alias
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-093" # retained historical alias
```

Important:

- Timeline remains a control-plane readback surface; this gate must not be satisfied by Swift-local orchestration history or artifact scans
- raw-detail handles are opaque daemon tokens and must be resolved only through the control-plane read API
- the latest retained historical alias live-agent Timeline remote UI proof is recorded at `docs/evidence/macos-operator-navigation/p093-remote-ui-proof-2026-05-22.json`

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

This gate name is a retained historical alias for the implemented server parity harness.

Scope:

- server parity `GoldenRunFixture` inventory and schema validation
- capture/regeneration lifecycle validation through `scripts/parity/capture-golden-run.sh --validate`
- deterministic offline replay over every required parity fixture
- per-fixture replay databases under `control-plane/target/parity/work/<generation_id>/<fixture_id>/parity.sqlite` (generation id comes from `P041_PUBLICATION_GENERATION_ID`, exported by the gate as `p041-<utc_iso>-<rand8>`; bare `cargo test` runs outside the gate use the sentinel `unscoped-fixture-replay`)
- per-fixture `BehavioralDiffReport` under `control-plane/target/parity/reports/<generation_id>/<fixture_id>/behavioral-diff-report.json` and `server-replay.json` under `control-plane/target/parity/work/<generation_id>/<fixture_id>/`
- per-fixture live-shadow reports under `control-plane/target/parity/shadow/<generation_id>/<fixture_id>/live-shadow-report.json` (schema `live-shadow-report.v1`)
- fixture-bound `surface_comparisons` for canonical state, projections, GraphQL readback, MCP report readback, artifact identity, and operator summary
- fail-closed shadow side-effect policy for stubbed runtime/provider inputs
- fixture-bound GraphQL run/stage/artifact/projection readback parity via `proposal_041_graphql_readback_parity_surfaces`
- fixture-bound MCP `reports.get` and `report://{run_id}` readback parity via retained P041-named tests
- runtime publication contract validation (row + detail schema agreement, provenance shape) at `control-plane/target/parity/publication/current/` plus generation-scoped staging at `control-plane/target/parity/publication/generations/<generation_id>/`; the gate computes the canonical status from fixture verdicts, provenance, and `parity-control` markers and promotes `ready_same_tree_verified` only when every fixture passes, the live tree is clean (`tree_clean == true` and `status_snapshot_line_count == 0`), and provenance is real (not the `test-run-no-git` sentinel)
- `control-plane/target/parity-control/` is acquired by the gate driver before destructive work; `lease.json`, `current-step.json`, `timeout-marker.json`, `interruption-marker.json`, `release-marker.json`, and `reclaim-marker.json` are written via the same-directory temp-file + Darwin `F_FULLFSYNC` (with `fsync` fallback) + atomic-rename contract. Before any `target/parity*` directory is created or written, the gate rejects symlinked or out-of-target paths.
- server parity subprocesses run through the gate supervisor with a dedicated process group/session. The supervisor records the active `pgid` in `lease.json` and `current-step.json`, enforces a 25-minute default gate deadline (`P041_GATE_DEADLINE_SECONDS=1500`) plus per-command deadlines for replay (`P041_REPLAY_DEADLINE_SECONDS=60`), GraphQL/MCP readback (`P041_READBACK_DEADLINE_SECONDS=30`), and live-shadow validation (`P041_SHADOW_DEADLINE_SECONDS=60`), with `P041_DRAIN_GRACE_SECONDS=30` for termination drain. Test binaries are prebuilt before the short per-fixture deadlines begin.
- On timeout, the supervisor sends process-group termination and publishes `blocked_timeout` through `timeout-marker.json` plus the current runtime row/detail. On SIGINT/SIGTERM interruption, it follows the same drain path and publishes `blocked_interrupted` through `interruption-marker.json` plus the current runtime row/detail.
- The reclaim matrix is implemented in the gate: Case A live/fresh owners fail closed as in-progress; Case A2 live/stalled owners park at `blocked_manual_recovery` after two unchanged heartbeat/control-sequence observations; Case B missing `pgid` metadata parks at `blocked_manual_recovery`; Case C observable descendants park at `blocked_manual_recovery`; Case D PID-gone plus proven descendant absence writes `reclaim_allowed`.
- generation retention follows the server parity retention contract: the gate prunes older non-manual generations oldest-first while preserving the newest ready generation, the newest non-manual blocked diagnostic generation, and every `blocked_manual_recovery` generation; storage above 500 MB triggers a `[WARN]` listing preserved roots and sizes (warning only, never authorization to delete manual-recovery evidence)

Use when:

- changing server/client parity fixture contracts
- changing Rust replay, projection, artifact/report, GraphQL-readback, or MCP-readback parity surfaces
- preparing P031 thin-client cutover evidence
- updating server parity golden fixtures or behavioral diff schema

Host policy:

- local Rust toolchain required
- no macOS UI target, simulator, daemon process, or live ACP/provider adapter required
- offline replay runs in per-fixture SQLite databases and temporary artifact roots

Command:

```bash
./scripts/test-gate.sh proposal-041
```

Important:

- `p041` is accepted as an alias
- the gate fails closed on missing fixtures, invalid schema, missing capture/regeneration provenance, missing executable frozen-input refs, missing fixture-bound surface comparisons, missing GraphQL/MCP collector owners, missing live-shadow correlation, or blocking divergences
- the runtime publication contract test asserts row/detail schema-version equality, status equality, `publication_generation_id` equality, and canonical-path validation for `runtime_detail_path` / `reference_detail_path`; live `ready_same_tree_verified` promotion (with real provenance, clean tree, and live-checkout match) is wired and gated by `scripts/p031-thin-ui-gate.py validate_p041_parity_row`
- the canonical P031 acceptance switch is the runtime row at `control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json`; the reference snapshot at `docs/reference/p031-p041-parity-evidence.json` is populated via `scripts/parity/promote-p041-reference.sh` after a successful gate run
- the `p031-p041-parity-evidence.md` companion is explicitly **non-authoritative** and structurally non-normative.

Status Vocabulary:

| Status enum | Meaning | CLI Prefix |
|---|---|---|
| `ready_same_tree_verified` | Ready for P031 acceptance/promotion | `PASS` |
| `blocked_manual_recovery` | Stale/ambiguous owner requires manual resolution | `FAIL` |
| `blocked_missing_evidence` | Missing producer artifact | `FAIL` |
| `blocked_divergence` | Behavioral regression in fixture/surface | `FAIL` |
| `blocked_dirty_tree` | Rerun required on clean checkout | `WARN` |
| `blocked_timeout` | Timed out; descendant absence not proven | `WARN` |
| `blocked_interrupted` | Interrupted by operator | `WARN` |
| `blocked_in_progress` | Rerun active; do not trust stale evidence | `INFO` |

Summary Grid:

- The gate renders a 7x6 matrix of fixtures by surfaces.
- Grid tokens: `PASS`, `FAIL`, `MISS`, `SKIP`, `TIMEOUT`.
- The CLI switches to a vertical narrow-terminal fallback if width is insufficient.

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

### `proposal-031|p031`

Historical gate alias for the implemented thin GraphQL-only UI inventory, static guard, and write-path guide gate.

Scope:

- Python script validation (`scripts/p031-thin-ui-gate.py`) for governed UI constraints
- verification of `docs/reference/p031-thin-ui-inventory.json`, `docs/reference/p031-operator-write-path-guide.json`, and `docs/reference/p031-phase-0-artifact-manifest.json`
- rust tests for `proposal_031_` prefix in `graphql-server`
- rust authorization tests `proposal_031_authorization` in `graphql-server`

Use when:

- changing the GraphQL-only thin macOS client boundary
- updating the UI inventory, operator write-path guide, or Phase 0 artifact manifest
- modifying GraphQL read/subscription authorization boundaries for the governed thin UI

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
- the manifest must represent the real closeout state and must not mark blocked evidence as `ready`
- dogfood sign-off details are outside this gate; this gate only requires the evidence artifact contract to be present and internally consistent

### `proposal-072|p072`

Historical gate alias for the implemented UI action boundary: approval-only GraphQL UI mutation boundary and MCP-only command routing.

Scope:

- composes `proposal-031`
- Swift unit tests for the governed GraphQL request boundary, including `approveApproval` / `rejectApproval`
- Rust domain/auth tests for the UI action routing registry and principal surface policies
- Rust GraphQL tests proving `ui_operator` can execute only approval mutations and is denied non-approval command mutations
- docs/inventory checks proving the stable thin UI boundary is reconciled with the approval-only exception

Use when:

- changing governed SwiftUI approval actions
- changing GraphQL mutation authorization or principal surface policies
- changing the thin UI/static UI action boundary or inventory
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

Historical gate alias for thin UI closeout readiness.

Scope:

- composes `proposal-031`
- verifies release evidence files are tracked, including report-payload runtime JSON and sanitized degraded-state screenshot evidence
- fails while the Phase 0 manifest, degraded-state evidence, freshness evidence, UX/accessibility evidence, or dogfood sign-off contain pending/template/limitation states that are not explicitly release-owner deferred
- fails while the dogfood checklist has unchecked items without an accepted release-owner deferral

Use when:

- deciding whether the retained thin UI boundary can be considered implementation-closeout ready
- reproving implementation readiness after dogfood/sign-off evidence or release-owner deferrals are updated

Host policy:

- same as `proposal-031`
- no live daemon required

Command:

```bash
./scripts/test-gate.sh proposal-031-readiness
```

Important:

- this is intentionally stricter than `proposal-031`
- `proposal-031` passing means the static/API/read-boundary contract is intact
- `proposal-031-readiness` passing means the retained thin UI closeout evidence is no longer known-pending

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

- this is the canonical proof path for the proposal-047 control-plane workspace slice
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

- this is the canonical proof path for [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md)
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

Command:

```bash
./scripts/test-gate.sh proposal-048
```

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
./scripts/test-gate.sh proposal-049
```

### `proposal-077|p077` - retained historical alias

Bounded implementation closeout readiness gate. The alias name is retained for
historical compatibility with existing scripts, tests, and receipts.

Scope (what this gate actually covers):

- static presence and required-field checks for retained historical alias evidence file `docs/reference/p077-rollout-dependency-evidence.md`
- static presence and required-field checks for retained historical alias evidence file `docs/reference/p077-closeout-readiness-ui-evidence.md`
- `implementation_closeout_readiness_v1` decision-matrix validation (Rust domain/db/engine)
- proposal gate domain contracts and status normalization
- audit verdict policy enforcement (synthesizer unit tests)
- bounded code/refine loop and soft convergence checkpoints (memory-level)
- closeout fingerprinting and latency budget validation
- DB closeout transaction atomicity (db integration tests)
- durable rollout metric, go/no-go decision, and rollback-to-advisory execution
  fixtures
- accessor routing proof (in-memory, not against live orchestrator graph)
- GraphQL readback parity through the canonical closeout-readiness accessor
- MCP `runs.get`/`runs.list` readback parity through the same accessor and
  exported projection shape

NOT covered by this gate (require additional integration gates or manual evidence):

- integrated orchestrator transition guard against live SQLite (state_9 real run)
- Swift workspace tests

Use when:

- changing implementation closeout readiness logic, status normalization, or decision rules
- changing proposal gate domain contracts or DB closeout transaction
- validating bounded loop convergence or soft checkpoint behavior at the synthesizer level
- reproving the closeout readiness Rust domain/db/engine slice

Host policy:

- local Rust target only; no Swift or UI build required

Command: retained historical alias `./scripts/test-gate.sh proposal-077`.

Important:

- this gate covers the Rust domain/db/engine slice and GraphQL/MCP readback
  parity of closeout readiness; remote macOS runtime proof is the companion UI
  gate
- it fails fast when the retained historical alias rollout/dependency evidence
  document is missing required dependency checklist, metric ledger, durable
  rollout store, or rollback fields
- it fails fast when the retained historical alias UI evidence document is
  missing token, contrast, diagnostics, recovery, route, or accessibility
  mappings
- it validates that a run cannot enter manual release without a resolved proposal gate or with pending code blockers
- it verifies the transition evaluation reads the active `implementation_closeout_readiness_v1` contract truth

### `proposal-077-ui|p077-ui` - retained historical alias

Remote macOS runtime proof for the closeout readiness surface.

Scope:

- direct closeout-readiness fixture launch
- compact signal activation into the full closeout card
- primary unblock and secondary blocker focus/read order
- diagnostics sheet open plus explicit return/backlink route
- generation-id copy command success or fallback feedback
- closeout readiness screenshot evidence capture

Host policy:

- remote macOS UI host required
- runs through the same signed XCUITest path as other repository UI gates

Command: retained historical alias `./scripts/test-gate.sh proposal-077-ui`.

### Retained Historical Alias: `proposal-078|p078`

Durable side-effect ledger gate for migration, CAS races, preflight, release lifecycle wiring, and MCP tools. The `proposal-078` and `p078` names are retained historical aliases after the proposal text was promoted into the reference docs.

Scope:

- side-effect domain models and status transitions
- Migration 052 round-trip and CHECK constraints
- Side-effect repository operations and CAS predicates
- DurableEffectCoordinator preflight blocking for unresolved effects
- Startup/watchdog recovery for expired executing side effects
- Startup/watchdog recovery for prepared, externally observed, and settled-evidence integrity windows
- Ledger readback circuit breaker: three readback errors in five minutes opens a ten-minute fail-closed breaker per call site
- Lease renewal and deadline wiring for long-running release side effects
- side-effect evidence spooling manifest contract, including `release-receipt.json`, `stdout.log`, `stderr.log`, `git-ls-remote.json`, `upload-readback.json`, `archive-summary.json`, `reconciliation-report.json`, and manifest-last validation
- Release retry integration coverage for stage retry, manual release retry, and targeted retry guards
- Native release lifecycle coverage for `git_commit`, `git_push`, `build_archive`, and `connect_upload`
- Executor start CAS (prepared -> executing) with race-condition verification
- Reaper transition (stale executing -> needs_reconciliation)
- MCP effects.* tools (list, inspect, reconcile, mark_conflict, mark_unrecoverable, clear_after_manual_verification)
- Authorization policy: all effects.* tools are operator-only (reads and mutations both gated to `PrincipalClass::Operator`, since `last_error` and evidence pointers may contain sensitive adapter output)
- GraphQL, MCP run report, rollout-contract fixture, and Swift read-only projections for side effects
- retained side-effect metric literals for the full rollout contract: durable-intent percent, intent, transition, retry block, readback error, circuit-open, recovery, settlement latency, unresolved gauges/age, evidence bytes/disk, and prepare-denied counters
- Recorded macOS accessibility/view-hierarchy proof for the read-only side-effect card and sidebar signal

Use when:

- changing side-effect persistence or status logic
- changing durable-retry preflight or reconciliation logic
- changing native release executor wiring or release receipt settlement
- changing MCP effects.* tools or authorization
- changing side-effect readback in GraphQL or reports
- changing rollout-contract side-effect readback fixtures or Swift read-only decoding

Host policy:

- local Rust toolchain required
- no UI host or simulator needed
- executes in-process against the control-plane/ workspace

Command: retained historical alias `./scripts/test-gate.sh proposal-078`.

Important:

- `p078` is accepted as a retained historical alias
- this is the canonical proof path for the durable side-effect ledger slice
- it validates that at most one external-write attempt is allowed per side_effect row
- it verifies the fail-closed preflight that blocks retries when unresolved effects exist
- it verifies the fail-closed circuit breaker that blocks retries when ledger readback repeatedly fails
- it verifies the wired release adapters and receipt/readback lifecycle without live external side effects
- it verifies startup/watchdog recovery, lease renewal markers, side-effect evidence spooling, public conflict disposition, and rollout/operator readback parity

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

Important:

- `p057` is accepted as an alias
- the active artifact index is canonical in SQLite; `active-index.json` is only an exported projection and stale/partial exports must never drive transition truth
- degraded partial-output settlement is denied by default and requires explicit compiled stage policy before `valid_outputs_from_failed_execution` can satisfy transitions
- typed operator overrides are separate from raw report files, require operator capability, write command journal evidence, expire at `expires_at_stage`, and remain visible in readback after expiry
- the gate fails closed if canonical status normalization, active-index SQLite ownership, stale export rebuild, raw artifact fallback denial, degraded policy, typed overrides, or GraphQL/MCP readback parity evidence is missing

### `proposal-058|p058`

Regression gate for ACP provider failure classification, session artifact ownership, and configurable escalation-chain proof.

The original ACP-classification proposal and the configurable escalation-chain proposal have been implemented and retired; the gate name remains `proposal-058|p058` because Swift/Rust test targets, migration symbols, and historical proof lanes use that identifier. This gate covers the implemented escalation policy schema, readback, metrics inventory, macOS read surface, and owned tier-selection behavior documented in [escalation-policies.md](escalation-policies.md).

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
- escalation_policy_v1 schema: `EscalationTierKind`, `EscalationPauseReason`, `EscalationTrigger` domain types
- escalation ledger, execution metadata, and event journal insert/read round-trips
- `redaction_version` required at insert time for every escalation event
- pause reason vocabulary coverage (all 13 catalog entries) and unknown-value round-trip
- malformed payload_json rejection at repository layer
- policy compile validation: `EscalationPolicyYaml` parser, deny_unknown_fields, backend_profile resolution, hash freezing into RunPlan
- ledger/agent-execution/metadata commit inside the engine-owned start transaction (`proposal_058_claim_start`), including insert-or-ignore idempotency for the chain row
- tier selection writes (`agent_execution_runtime_facts.would_select_*`) populated from the frozen `RunPlan` policy at agent-execution completion (`engine/src/shadow_escalation.rs`)
- durable scheduler readback fields derived from redacted `escalation_events` (`waiting_retry_after_until`, `escalation_trace_json_redacted`, `external_acknowledgement_ref`, `feature_flag_state`, and per-attempt `digest_inputs`)
- governed macOS read-surface components compile and are covered by focused Swift tests (`EscalationStatusCapsule`, `EscalationBannerStack`, `EscalationLineageView`, `EscalationPauseCard`, `EscalationTraceTimeline`, `DriftReviewSheet`, MenuBarExtra overflow routing, retained shared adapters, command disabled-state parity, actual drift sheet tier/trigger/max-attempt inputs, compact banner co-occurrence summarization, non-collapsed lineage disclosures, pause-card ultra-narrow fallback, SF Symbol availability, drift diff presentation, and read-only trace pasteboard copy)
- full escalation metric inventory declaration plus production emission from ledger/event writes for the metrics backed by durable escalation state
- idempotency-key uniqueness enforced by migration `078_p058_escalation_idempotency.sql` (one chain per `run_id`/`stage_id`/`agent_id`/`policy_id`; one execution-metadata row per `ledger_id`/`tier_id`/`tier_attempt_index`)

Use when:

- changing ACP provider/transport error classification
- changing executor output validation settlement or degraded output behavior
- changing session reuse, retry supersession, or late-output handling
- changing artifact active-index source provenance
- changing GraphQL or MCP execution truth readback
- changing escalation domain types, repository layer, or GraphQL escalation readback
- changing escalation_policy_v1 YAML parsing, compile validation, or RunPlan policy snapshot

Host policy:

- local Rust and macOS Swift test toolchains required
- no live provider account, simulator, daemon process, UI target, network, or real quota exhaustion required
- remote visual/runtime, Full Keyboard Access, contrast/reduced-motion, and live operational drill evidence is owned by [P096](../proposals/096-p058-release-evidence-and-macos-runtime-proof.md), not this implementation gate

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
- the gate fails closed if runtime facts, source-generation claims, pending retry supersession, artifact provenance, GraphQL/MCP runtime-facts parity, or escalation schema evidence is missing
- escalation events must supply a `redaction_version` stamp; insert without one must fail

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

### `proposal-066|p066`

Historical gate alias for the implemented provider toolchain cache mapping contract.

Scope:

- Swift guardrail proving `ToolchainMappingReadAdapter` owns toolchain policy decoding for operator-facing consumers
- workflow catalog schema and frozen snapshot compatibility checks for `toolchain_cache_policy`
- domain failure-kind coverage for setup failure and Xcode queue timeout
- database migration and `actual_toolchain_mapping_diagnostics_json` persistence coverage
- GraphQL, MCP, and report readback synthesis for active, disabled, unsupported, failed, queued, and legacy mapping states
- Xcode host-executor argument and `TMPDIR` rewriting, Go environment mapping, and per-run Xcode lease behavior
- startup recovery and housekeeping readbacks for toolchain cache cleanup
- migration drill coverage for legacy NULL rows and post-migration diagnostics rows

Use when:

- changing `toolchain_cache_policy` schema or frozen snapshot compatibility
- changing ACP toolchain directory preparation, Xcode host-executor mapping, Go cache environment shaping, or per-run Xcode lease behavior
- changing GraphQL/MCP/report exposure of `actualToolchainMappingDiagnostics`
- changing startup recovery or housekeeping readbacks for toolchain cache roots

Host policy:

- local Rust toolchain required
- no live provider account, simulator, daemon process, UI target, or network required

Command:

```bash
./scripts/test-gate.sh proposal-066
```

Important:

- `p066` is accepted as an alias; the historical gate name is retained for stable automation compatibility
- this gate proves the Phase 0 scaffold and readback contract, not dogfood promotion for Xcode or Go defaults

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

- the gate fails closed if runtime facts, source-generation claims, pending retry supersession, artifact provenance, GraphQL/MCP runtime-facts parity, or escalation schema evidence is missing
- escalation events must supply a `redaction_version` stamp; insert without one must fail

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

### `proposal-085|p085`

Thin-client read-model parity and affordance contract gate.

The original proposal document is retired after implementation. Operational truth lives in
[thin-client-read-model-affordance-contract.md](thin-client-read-model-affordance-contract.md);
the `proposal-085` and `p085` names remain as retained proof aliases.

Scope:

- `docs/reference/thin-client-read-model-affordance-contract.md` exists and contains `thin_client_affordance_contract_v1` with all required affordance rows (`artifact.preview.listLabel`, `artifact.preview.detail`, `report.payload.metadata`, `freshness.badge.run/stage/approval/artifact`, `approval.resolve.approve/reject`, `diagnostic.copy`, `external.command.placeholder`)
- All eight p085 negative fixture files exist as valid JSON with a `contract_violation` field and scenario-specific semantic expectations
- `control-plane/crates/graphql-server` runs the P085 backend proof slice (`proposal_085_`) covering approval projection fields, artifact/report payload projection states, authorization denial for diagnostic fields, P081 boundary row linkage, and typed `conflictResultCode` readback with a real failed `command_journal` id for both approval mutations
- `Chainworks Forge/Support/P085AffordancePresenter.swift` exists with immutable Equatable Sendable DTOs: `P085ArtifactAffordanceState`, `P085ApprovalAffordanceState`, `P085FreshnessAffordanceState`, `P085DiagnosticAffordanceState`; `canDrivePayloadAvailability` and `canDriveApprovalActionability` are always `false`; `mergedAffordance` enforces stale-detail guard; `payloadPresentation(fromRaw:)` and `P085FreshnessState.fromRaw` map unknown enum strings to `.unknown`
- `Chainworks ForgeTests/Proposal085Tests` runs as the Swift parity slice proving: `payload_deferred` → `.deferred` (not `.unavailable`); `metadata_only` → `.metadataOnly`; unknown enum strings → `.unknown`; stale async detail does not overwrite newer selection; approval actionability requires `writePathState == .available` and matching `availableActions`; freshness is diagnostic-only; diagnostic affordance invalidates on unauthorized freshness

Use when:

- Changing `P085AffordancePresenter` mapping logic or DTOs
- Adding a new GraphQL-driven Swift affordance (must add a contract row first)
- Verifying that `payload_deferred` and `metadata_only` are represented honestly

Host policy:

- local Swift toolchain (Xcode 26.3+), Rust toolchain, and Python 3 required; no UI host, daemon, or network required
- the Rust slice is a backend GraphQL contract proof and must pass before the Swift presenter slice runs

Command:

```bash
./scripts/test-gate.sh proposal-085
./scripts/test-gate.sh p085
```

Important:

- `p085` is accepted as an alias; both run the same proof slice
- the gate fails closed if the contract doc is missing a required row, P081 boundary row, or term; any negative fixture is absent or fails its scenario-specific semantic expectation; the backend GraphQL proof is missing/failing; the presenter file is missing a required symbol; or the `Proposal085Tests` Swift slice fails
- `payload_deferred` must never collapse to `unavailable`; enforced by the Swift test slice
- unknown GraphQL enum values must produce `.unknown` states; proved by `unknownPayloadStateFailsClosed` and `unknownFreshnessStateFailsClosed` tests

### `proposal-087|p087`

Retained historical alias for the local storage tiering, read-path liveness, and SQLite exit-criteria gate. Operational truth lives in [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md) and [rust-control-plane.md](rust-control-plane.md).

Scope:

- verifies the storage-tiering migrations, projection freshness schema, hot-read circuit state, maintenance-operation readback, invalidation wiring, and typed MCP storage-health error contract
- validates retained evidence fixtures under `docs/evidence/p087/api/`, `docs/evidence/rollout-contract/operator-readback/p087-storage-tiering-full-surface.fixture.json`, and `docs/evidence/rollout-contract/negative/p087-*.json`
- runs focused Rust slices for DB, auth, engine, MCP, and GraphQL storage-health behavior plus Swift source checks for projection-lag and daemon lifecycle diagnostics readback

Command:

```bash
./scripts/test-gate.sh proposal-087
./scripts/test-gate.sh p087
```

Important:

- `proposal-087|p087` is retained as a historical gate alias; the retired proposal document is not the source of operational truth
- `p087_*` remains stable rollout-readback vocabulary for this implemented contract
- the gate is a focused proof path for storage tiering and read-path liveness, not a substitute for the full repository gate

### `proposal-088|p088`

Code-writer completion handoff, output freshness, and repair diagnostics gate.

Scope:

- deterministic P088 evidence fixtures exist under `docs/evidence/088-code-writer-completion/`
- fixtures cover P087 terminal-completed missing outputs, the 70c9-shaped preexisting dirty-work timeout negative case, large streamed prelude/tail capture, prompt-side evidence, public enum unknown handling, normal materialization without repair, mutation-guard failure, docs-only eligibility, generated-evidence-only ineligibility, ingestion-boundary failures, partial-write recovery, and `worktree_fingerprint_v1`
- Rust focused tests cover ACP capture, typed code-writer completion session events, prompt-level runtime receipt persistence, canonical receipt-link readback, receipt linkage/conflict detection, worktree fingerprint classification, completion receipt readback, GraphQL/MCP/run-report `implementationCompletion` parity with closed vocabularies plus `known=false` unknown-value metadata, and a stale `implementation_active`/P037 idle-terminalization canary proving `code_writer` candidates enter P088 receipt readback instead of active-prompt auto-requeue

Command:

```bash
./scripts/test-gate.sh proposal-088
./scripts/test-gate.sh p088
```

Important:

- completion repair eligibility must require `work_change_kind=current_attempt_diff`; inherited dirty work must fail closed as ineligible
- stale `implementation_active` proof is a focused executable engine canary for the actual `InvokeAgent` error branch; it does not claim a full live ACP supervisor timeout end-to-end
- usable final `CHAINWORKS_OUTPUT` must materialize through normal settlement without completion repair
- public readback must preserve known enum values and fail closed for unknown future values
- `implementationCompletion` must be projected from canonical linked receipt truth, not from latest historical receipt by timestamp
- required evidence write failures must surface as `completion_receipt_partial_write` / `storage_write_failed`, not clean missing fields

### `proposal-089|p089`

Retained historical alias for the Junie structured-output capability and ACP
`code_writer` canary evidence gate. Operational truth lives in
[acp-runtime-transport.md](acp-runtime-transport.md) and
[structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md).

Scope:

- native Junie standard CLI evidence under `docs/evidence/089/junie-structured-output-canary/native/` for strict JSON, strict `CHAINWORKS_OUTPUT`, and repair-style minimal synthesis
- ACP canary evidence under `docs/evidence/089/junie-structured-output-canary/acp-canary/`
- production `code_writer` catalog binding readback: `agent_id=code_writer`, `backend_profile=junie_code_editor_acp`, `provider=junie`, `runtime_profile=junie_cli_acp`, `model=junie-default`, and `effort=high`
- full production output set: `implementation_progress`, `implementation_self_assessment`, `changed_files_manifest`, and `tests_result`
- ACP completion capture, extraction, no-truncation, and no-repair metadata
- engine-owned settlement/materialization through `generate_changed_files_manifest_if_declared` and `settle_agent_outputs_from_discovery_decisions`
- `changed_files_manifest` classification as control-plane-generated evidence that does not contribute to Junie capability
- `worktree_fingerprint_v1` mutation guard readback, clean-worktree signoff semantics, and diagnostic dirty-work mode
- one-way hash linkage between `evidence-index.json` and `live-gate-run.json`
- proof-critical file hash validation for the gate, refresh script, Junie adapter, ACP transport, canary harness, executor, fingerprint classifier, and production agent catalog
- negative fixtures for broad allowed roots, proof-critical drift, invalid `changed_files_manifest` source attribution, and dirty diagnostic mode

Host policy:

- default mode is local evidence validation; no live provider invocation is required
- live mode requires the configured Junie toolchain and writes refreshed evidence

Command:

```bash
./scripts/test-gate.sh proposal-089
./scripts/test-gate.sh p089
```

Live evidence refresh:

```bash
CHAINWORKS_PROPOSAL_089_LIVE=1 ./scripts/test-gate.sh proposal-089
```

Diagnostic dirty-work mode:

```bash
CHAINWORKS_PROPOSAL_089_ALLOW_DIRTY=1 ./scripts/test-gate.sh proposal-089
```

Important:

- the live canary uses a disposable worktree rooted under `.chainworks/tmp/p089-*`
- diagnostic dirty-work mode writes non-signoff mutation readback and must exit non-zero
- a default-mode pass requires checked-in evidence from a prior successful live-mode gate
- the proof is intentionally narrow: it proves Junie structured-output capability plus the small ACP `code_writer` handoff boundary, not full scheduler/projection behavior and not a blanket fix for long-running P036-family failures

### `proposal-090|p090`

Retained historical alias gate for Junie `code_writer` runtime-hardening evidence inventory.
Short label: Junie runtime-hardening evidence inventory.

Scope:

- validates `docs/evidence/090/junie-runtime-hardening/evidence-index.json`
- verifies every P090 completion-boundary subtype has one historical or synthetic evidence row
- verifies every fixture path exists, has schema `p090_subtype_fixture_v1`, and matches its SHA-256 in the evidence index
- verifies the public subtype contract is a provider-neutral wrapper with unknown/raw round-trip behavior
- verifies required negative fixture classes have concrete files and SHA-256 entries for provider-authored failure spoofing, identity mismatch, unknown envelope schema, malformed repair sibling overwrite, and permission-denied preflight no-launch behavior
- verifies checked-in long-running Junie refine-like canary evidence produced through `BackgroundExecutor.process_next_item`, with strict final payload, enforced preflight, fresh settled outputs, and Junie ACP receipt readback
- verifies stable reference docs name the engine-owned failure authority, durable per-output settlement rows, preflight lifecycle, post-preflight provider capacity boundary, rollout flags, and retained gate alias
- runs focused Rust proof tests for receipt/readback subtype persistence, settlement-row idempotency, provider-authored failure-envelope spoof rejection, Junie no-launch preflight classification, staged repair materialization, and GraphQL/MCP readback parity

Command:

```bash
CHAINWORKS_JUNIE_ACP_BINARY=/Users/user/.local/bin/junie CHAINWORKS_PROPOSAL_090_LIVE=1 ./scripts/test-gate.sh proposal-090
./scripts/test-gate.sh proposal-090
./scripts/test-gate.sh p090
```

Important:

- this gate now combines evidence inventory validation with focused implementation checks; it is not a substitute for full workspace `cargo test` or the broader app gates
- `proposal-090|p090` is retained as a historical gate alias; operational truth lives in [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md#junie-code-writer-completion-boundary), [acp-runtime-transport.md](acp-runtime-transport.md), and [rust-control-plane.md](rust-control-plane.md)
- live mode refreshes `docs/evidence/090/junie-runtime-hardening/refine-like-canary/`; default mode requires that checked-in evidence and fails closed if it is missing or stale
- the Rust checks are intentionally scoped to P090 contract behavior and the P088 readback surfaces that expose it
- P090 keeps `completion_boundary_subtype` as a provider-neutral public wrapper; the initial known values are Junie-prefixed because the first covered failure family is Junie ACP

### `proposal-091|p091`

Retained historical alias for the targeted retry authority evidence inventory and focused runtime proof. Operational truth lives in [rust-control-plane.md](rust-control-plane.md#targeted-retry-authority).
Short label: targeted retry authority runtime proof.

Scope:

- validates `docs/evidence/091/targeted-retry-authority/evidence-index.json`
- verifies the P086 orphaned retry readback fixture exists, has schema `p091_orphaned_retry_readback_fixture.v1`, and matches its SHA-256 in the evidence index
- verifies the fixture preserves the historical orphan shape: pending retry, no live work items, no active agent executions
- verifies stable reference docs include the closed contract decisions for targeted-agent `InvokeAgent`-first lifecycle, historical orphan recovery provenance, `stale_retry_recovered`, DB-enforced active authority uniqueness, target-aware work-item repository semantics, authority history readback, startup recovery ordering before projection rebuild/catch-up enqueue, partial-target payload fail-closed semantics, rollout controls, and retained gate naming
- runs focused domain payload parsing tests, DB authority/projection tests, retry engine integration tests, GraphQL readback coverage, and MCP authority-history readback coverage

Command:

```bash
./scripts/test-gate.sh proposal-091
./scripts/test-gate.sh p091
```

Important:

- this gate now combines evidence inventory validation with focused implementation checks; it is not a substitute for full workspace `cargo test` or broader app gates
- `proposal-091|p091` is retained as a historical gate alias; the retired proposal document is not the source of operational truth
- full-stage retry is `AdvanceRun`-first; targeted-agent retry is `InvokeAgent`-first
- historical orphan recovery settles as `status = skipped` with `terminal_reason = stale_retry_recovered` and a non-active recovered authority provenance row
- `terminal_reason` is stage-owned and authority-history-owned for recovered orphan rows

### `proposal-092|p092` retained historical alias

Retained historical alias for the implemented retry authority payload target invariants and recovery contract. Operational truth lives in [`rust-control-plane.md#retry-payload-target-invariants-and-recovery`](rust-control-plane.md#retry-payload-target-invariants-and-recovery).
Short label: retry payload target invariants runtime proof.

Scope:

- verifies the stable Rust control-plane reference owns the current-target/provenance split, targeted retry sanitizer, post-invoke fail-closed behavior, startup/live recovery, and retained gate naming
- verifies the retained alias does not reuse the Junie runtime-hardening `proposal-090|p090` identity
- verifies the implementation covers all targeted retry producers, including auto-contract retry and `CommandHandler::RetryAgentExecution`
- verifies bounded startup and live recovery entry points are owned directly by the recovery service and daemon watchdog
- verifies durable `retry_payload_recovery_events` storage is the backing source for GraphQL, MCP, and report readback
- verifies rollout controls for diagnostic/enforce mode, disable switch, live batch limit, cooldown, counters, and idempotent repair keys are specified
- verifies startup diagnostic/enforce behavior is explicit and diagnostic startup does not fall through to generic abandoned-invoke blind retry for retry payload recovery candidates
- verifies GraphQL, MCP, and report readback schema placement for `retryPayloadRecovery` / `retry_payload_recovery`
- runs focused runtime tests for forward producer sanitization, post-invoke hard mismatch, durable recovery event storage, configurable live batch limiting, explicit `excluded_total` diagnostics, startup diagnostic/enforce recovery, GraphQL/MCP nullable missing-authority readback, and daemon live-hook compilation

Command:

```bash
./scripts/test-gate.sh proposal-092  # retained historical alias
./scripts/test-gate.sh p092          # retained historical alias
```

Important:

- this gate proves the retry payload recovery runtime contract with focused Rust tests; it is not a full workspace regression substitute
- `proposal-092|p092` is a retained historical alias; the retired proposal document is not the source of operational truth
- the alias does not replace P091 targeted retry authority or the retained Junie runtime-hardening `proposal-090|p090` gate
- live recovery is intentionally bounded to active runs with running `invoke_agent` work items and active retry authority, and defaults to diagnostic mode

### `proposal-075|p075`

Retained historical alias for the local persistence write budget, evidence spooling, storage diagnostics, and fail-closed registry gate. Operational truth lives in `docs/reference/rust-control-plane.md`.

Scope:

- `write_class` types: `WriteClass`, `WriteOperation`, `WriteResult`, `SpoolWriteOutcome`
- `writer`: `DbWriter` constants, lane order, bounded MPSC executor — biased priority drain (`CriticalBarrier`/`OperatorCommand` polled before lower lanes), enqueue-to-commit deadline accounting (`WriteTimeout`), busy-error classification (`WriteBusyExhausted`), 1 Hz heartbeat with `is_alive()`, lane starvation watchdog incrementing `lane_starvation_total`, graceful shutdown that rejects new B/C/D writes and drains Class A within `SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS`, populated `SHUTDOWN_ADMITTED_OPERATIONS` allowlist (terminal canonical operations admitted; unlisted Class A writes denied), Class B coalescing buffer (`COALESCE_FLUSH_INTERVAL_MS=500` with unconditional drain-all per tick, `COALESCE_FLUSH_MAX_MERGES=64`, last-writer-wins, `COALESCE_MAX_KEYS=1024` saturation reject with `coalescing_map_saturated`, force-drained on shutdown), per-lane oldest-enqueued tracking populating `WriteRejected.oldest_queued_ms`, and storage health readback units/freshness/kill-switches.
- `evidence_spool` (Phase 3): `write_spool_file` ordering proof (temp write → SHA-256 → `fsync(file)` → atomic no-replace commit → `fsync(parent_dir)`), `verify_spool_file` reader integrity, `sweep_evidence_orphans` walk that backfills `recovered_orphan` metadata for crash-orphaned files and is idempotent on a second pass, sweep budget enforcement (`max_files`/`max_bytes` truncate the pass and set `OrphanSweepReport.truncated`, with over-budget candidates skipped before read), `run_id` filter that skips files outside the requested run, `dry_run` mode that scans and stream-hashes checksums but does not insert metadata rows, high-volume "one metadata row per logical object" proof against `evidence_spool_refs`, and security regressions: canonical-layout enforcement (`evidence/runs/...` required — P075-SEC-002 layout), write-time `run_id` path-ownership binding (P075-SEC-001 / H-002), no-clobber commit (identical bytes = idempotent retry; differing bytes = hard error — P075-SEC-002 no-clobber), symlink-escape rejection via canonicalized root plus per-segment symlink-safe parent walk that refuses to mkdir through any symlinked component, with `verify_spool_file` and orphan sweep using no-follow `symlink_metadata` on candidates (P075-SEC-H001), Unix file mode `0o600` and directory mode `0o700` (P075-SEC-H002)
- `bypass_allowlist`: parser, expiry, canonical file validation
- `operation_registry`: parser, validation, canonical file validation
- `evidence_spool_refs`: migration and repository round-trips, CHECK constraints, `validate_relative_path` symmetry on read/write, `validate_path_ownership` run-id binding (P075-SEC-001), `canonicalize_summary_json` allowlist, identity-string control-character rejection
- `storage_write_pressure_snapshots` migration, validation, repository readback, and storage health freshness classification
- MCP/GraphQL storage diagnostics: typed `storageHealth`, `storage.health`, `storage.write_pressure`, `storage.evidence_spool_summary`, and `storage.reconcile_evidence_orphans`, including operator-only capability enforcement, fail-closed stale/degraded readback when no live writer heartbeat is supplied, and live `DbWriter` heartbeat readback when the daemon-owned writer is injected (`storage_health_with_writer` populates `writer.alive`/`totalQueued`/`lanes`/`lastHeartbeatAt`/`lastDrainAt`/lock wait p50/p95/transaction p50/p95/`isStale=false` from the live snapshot — covered by `proposal_075_storage_health_reads_live_dbwriter_heartbeat` in `graphql-server`, `storage_health_reads_live_dbwriter_heartbeat_when_injected` in `mcp-server`, and the file-backed WAL/lock canary `storage_health_file_backed_canary_reports_lock_wal_and_writer_metrics` in `db`)
- typed MCP storage error contract (`error: true`, `errorCode`, `message`, `tool` envelope) covering `invalid_input` (`reconcile_evidence_orphans_returns_invalid_input` for non-dry-run without `runId`), `stale` (`storage_health_returns_typed_stale_error`), `unavailable`, `maintenance_disabled`, and `unauthorized` through the real `tools/call` dispatch path (`proposal_075_storage_tool_dispatch`)
- P075 evidence docs must have no `pending_live_canary` marker in `docs/evidence/p075/phase1-baseline.md` and must include the high-volume producer inventory at `docs/evidence/p075/producer-inventory.md`
- fail-closed registry enforcement: `write-bypass-allowlist.toml` and `write-operation-registry.toml` must exist, be valid, include retirement metadata, reject all `temporary_rollout` bypass rows, reject production runtime transaction paths that bypass DbWriter-owned entrypoints, and cover observed non-test DbWriter operation names under `control-plane/crates/db/src`, `control-plane/crates/engine/src`, and `control-plane/crates/mcp-server/src`

Use when:

- changing write classes, priority lanes, or `DbWriter` constants
- updating the DB write-bypass allowlist or write-operation registry
- changing evidence spool references or storage write-pressure snapshots

Host policy:

- local Rust toolchain required; no UI target or simulator needed

Command:

```bash
./scripts/test-gate.sh proposal-075
```

Important:

- `p075` is accepted as an alias
- this is a fail-closed persistence contract gate, not an inventory-only check
- startup orphan reconciliation is available through the storage MCP diagnostic tool; daemon startup scheduling and telemetry producer extensions must keep this gate green

### `proposal-076|p076` retained historical alias

Auto-retry observation ledger contract, fixture, readback, and rollup proof gate. These retained historical aliases validate the observe-only invariant and exercise the MCP `automation.auto_retry.latest` readback source plus rollup tooling against a temporary JSONL ledger.

Scope:

- `auto_retry_observation_v1` strict-mode validation: required top-level fields, `observation_id` format `ar_obs_<UTC basic timestamp>_<12 lowercase hex>`, RFC3339 `observed_at`, closed `blocker_class` / `policy_decision` / `retry_action` / `retry_result` / `known_issue_status` / `budget_status` / `diagnostic_severity` / `readback_policy_status` enum domains, `additionalProperties=false` rejection of unknown fields, nested `budget_ref_v1` and `observation_summary_v1` shapes (required fields, nullable fields, scalar types)
- observe-only policy enforcement: ledger-created observations must have `retry_action` in `{none, recommend_retry}` and `retry_result` in `{not_attempted, not_allowed}`; any side-effecting retry dispatch in an auto-retry ledger record is a gate failure
- `auto_retry_readback_v1` six top-level required path fields: `ledger_path`, `budget_state_path`, `known_issue_catalog_path`, `generated_markdown_catalog_path`, `lock_path`, `rollup_report_path`
- `version_negotiation` object shape and JSON-RPC `unsupported_version` application error (`code: -32076`, populated `error.data` with `code`, `supported_versions`, `unsupported_versions`, `requested_versions`, no partial success payload)
- degraded-success readback (artifact-read degradation returns successful response with diagnostics + empty arrays, not a transport error) and `no_observation_history` null-readback path
- rollup grouping by `blocker_signature_id` from valid fixture records and from `scripts/chainworks/auto_retry_rollup.py`
- focused Rust test proving `automation.auto_retry.latest` reads a real JSONL ledger from the resolved meta-root and returns latest-by-run readback
- retained historical alias negative fixture inventory under `docs/evidence/rollout-contract/negative/` — twenty `p076-*` fixtures covering side-effect retry presence, human-gate retry, missing schema field, invalid enum, ledger-append missing newline / not fsynced, missing `budget_ref_v1` / `observation_summary_v1` / readback `lock_path` / `rollup_report_path`, budget failure retried, backpressure exceeded, human-gate starvation, orphaned planned attempt not escalated, PID-reuse lock liveness gap, poll timeout without observation, retry timeout duplicate not suppressed, markdown catalog as authority, unsafe stale-lock recovery, and unknown-field-strict rejection
- retained historical alias operator-readback fixture at `docs/evidence/rollout-contract/operator-readback/p076-full-surface.fixture.json` covering all required `operator_readback_v1` decision fields plus the auto-retry readback projection

Use when:

- changing auto-retry normative schemas (`auto_retry_observation_v1`, `auto_retry_readback_v1`, `auto_retry_budget_v1`, `auto_retry_known_issues_v1`, `budget_ref_v1`, `observation_summary_v1`, `common_diagnostic_v1`) or their enum domains
- adding, renaming, or removing retained historical alias negative fixtures or the operator-readback fixture
- changing the observe-only monitor, JSONL ledger, budget store, MCP readback, or rollup tool
- referencing the auto-retry observation ledger from dependent proposals (contract-aware repair, stale execution reconciliation, retry test matrix, temporary artifact lifecycle, retry authority)

Host policy:

- Python 3 fixture/rollup validation plus focused Rust MCP readback test; no Swift, app launch, or daemon process required

Command:

```bash
./scripts/test-gate.sh proposal-076  # retained historical alias
./scripts/test-gate.sh p076          # retained historical alias
```

Important:

- `p076` is accepted as a retained historical alias
- this is an observe-only proof gate; landing side-effecting retry remains out of scope and requires a later proposal
- the auto-retry ledger is observe-only by design — the gate enforces that ledger records do not record side-effecting retry dispatch and that human-gate retries are absent
- canonical JSON known-issue catalog is authoritative; markdown is generated-only and the retained historical alias negative fixture `p076-markdown-catalog-as-authority.json` proves this boundary
- implemented readback and rollup behavior is documented in `docs/reference/auto-retry-observation-ledger.md`

### `proposal-084|p084` retained historical alias

Executable rollout gates and observability contract gate.

Scope:

- `docs/reference/executable-rollout-gate-template.md` exists and contains required sections for all three v1 contracts, cutover policy, and security/path guidance (AC-001)
- `scripts/lint-rollout-contract` pure validator exists and correctly rejects all four linter-testable negative fixtures:
  - `docs/evidence/rollout-contract/negative/missing-hold-and-rollback.json` — fails `missing_hold_conditions` and `missing_rollback_disposition`
  - `docs/evidence/rollout-contract/negative/missing-metrics-p017-style.json` — fails `missing_metrics` (P017-style omission caught before closeout)
  - `docs/evidence/rollout-contract/negative/missing-operator-decision-fields.json` — fails `empty_readback_fields`
  - `docs/evidence/rollout-contract/negative/invalid-cutover-applicable-to.json` — fails `invalid_cutover_policy.applicable_to`
  - `docs/evidence/rollout-contract/negative/unsafe-path-and-command.json` — fails `unsafe_command` and `unsafe_path`
- Documentation-only negative fixtures exist as valid JSON where they describe runtime behavior rather than linter input (AC-006 self-contract check)
- Rust rollout-contract regressions run under the canonical gate: `cargo test -p engine rollout_contract_preflight --lib`, `cargo test -p db rollout_contract_checks --lib`, clean DB migration install, and schema-version parity. The Python phase also verifies the orchestrator keeps the rollout preflight hold path before code_writer enqueue and blocks the stage/run on `RolloutContractPreflightAction::Hold` (AC-005)
- The retained historical alias fixture `docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json` contains all 18 required `operator_readback_v1` decision fields and a `parity_lanes` object whose `mcp` and `release_receipt` payloads carry the same fields and whose `graphql` payload carries the matching camelCase projection fields (AC-004, AC-006)
- `Chainworks ForgeTests/Proposal084Tests` runs as the Swift parity slice, proving `RolloutDecisionSummary` decodes `operator_readback_v1`, `PreflightReport` carries the read-only summary, and the GraphQL run-row read model decodes the camelCase rollout readback without recomputing authority (AC-004, AC-006)
- This gate documentation section exists in `docs/reference/test-gates.md` and references `rollout_contract_v1`, `negative fixture`, and `lint-rollout-contract` (AC-002)

Use when:

- Changing the rollout gate template, linter logic, or fixture inventory
- Proving rollout-contract compliance on the current tree
- Verifying that unsafe inputs, missing metrics, and missing hold/rollback are rejected with bounded reasons

Host policy:

- local Python 3, Rust toolchain, and Swift Testing host (Xcode toolchain) required for the `Proposal084Tests` slice; no UI host or network required after Swift package cache is warm
- pure file-system + subprocess validation for the lint and fixture phase; no daemon process required

Command:

```bash
./scripts/test-gate.sh proposal-084 # retained historical alias
./scripts/test-gate.sh p084 # retained historical alias
```

Important:

- `p084` is accepted as a retained historical alias
- the gate runs `scripts/lint-rollout-contract` via subprocess; linter exit-0 on a negative fixture is a gate failure
- documentation-only self-contract fixtures are validated for JSON well-formedness only; linter-testable scheduler and cutover fixtures are linter inputs
- the gate validates parity-lane fixture shape (run_report, mcp, release_receipt, graphql), Rust rollout-contract preflight/storage regressions, clean migration install, and the Swift read-only presentation slice
- the gate fails closed if the template is missing a required term, any negative fixture is absent or malformed, the retained historical alias `p084-full-surface` fixture omits a required readback field or parity-lane payload, or the `Proposal084Tests` Swift slice fails

### `proposal-081|p081`

Boundary-first API and auth contract matrix gate (matrix/fixture validation, shared `BoundaryPolicy`, bounded `boundaryRuntime` and `operatorAlerts` readback, GraphQL WebSocket close-code enforcement, GraphQL `extensions.redactions`, `approval_mutation_idempotency`, Swift approval action attempt persistence, and `mcp_command_idempotency` dispatcher enforcement for state-changing MCP tools).

Operational truth lives in [boundary-first-api-auth-contract.md](boundary-first-api-auth-contract.md)
and the executable fixture [boundary-first-api-auth-contract.json](boundary-first-api-auth-contract.json);
the macOS-side contract lives in [swift-macos-boundary-contract.md](swift-macos-boundary-contract.md).

Scope:

- `scripts/check-boundary-coverage.sh` (also runs from `guardrails`)
- fixture validity: `boundary-first-api-auth-contract.json` parses, declares `schema_version == 1`, carries a `matrix_id`, and contains all 11 required row ids (`p081.ui_operator.graphql_query.read`, `p081.ui_operator.graphql_subscription.subscribe`, `p081.ui_operator.graphql_mutation.approval_action`, `p081.agent_operator.mcp_initialize.capability`, `p081.agent_operator.mcp_tools_list.discovery`, `p081.agent_operator.mcp_tools_call.command`, `p081.automation.mcp_tools_list.discovery`, `p081.automation.mcp_tools_call.command`, `p081.observer.mcp_tools_call.compact_read`, `p081.observer.graphql_query.read_only_opt_in`, `p081.developer_break_glass.debug_endpoint.disabled`)
- `boundary-first-api-auth-contract.md` exists
- `cargo test -p auth boundary::` — fixture loader, validator error codes, embedded last-known-good fallback into `read_only_safe_mode`
- `cargo test -p auth caller_class` — `CallerClass` enum, `CallerContext.caller_class`, and resolver
- `cargo test -p auth bootstrap_emits_schema_version_3` and `v3_principal_table_rejects_unknown_schema_version` — principal-table v3 parser
- `cargo test -p db repos::audit_log::` — `audit_log` repository write/append/checkpoint semantics
- `cargo test -p graphql-server proposal_081_boundary_runtime_graphql_readback_is_bounded` — bounded GraphQL `boundaryRuntime` operator diagnostic lane for active policy mode, safe-mode state, and audit-log health
- `cargo test -p graphql-server test_graphql_ws_rejects_missing_connection_init_auth` and `test_graphql_ws_rejects_non_ui_caller_with_forbidden_close` — GraphQL WebSocket pre-auth close-code contract (`4401`, `4403`, `4408`)
- `RUST_MIN_STACK=8388608 cargo test -p mcp-server proposal_081_runtime_health_includes_boundary_runtime_readback` — bounded MCP `runtime.health.boundaryRuntime` diagnostic lane with the same audit-log health envelope
- `RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_ideas_create_records_command_journal_and_idempotency_linkage` and `p081_ideas_create_idempotency_replay_does_not_duplicate_command_commit` — MCP state-changing idempotency requires a durable command-journal link and duplicate replays do not duplicate command/domain rows
- `Chainworks ForgeTests/Proposal081ApprovalActionAttemptStoreTests` — Swift approval action attempt store reuses one idempotency key across retries/restarts, scopes by approval action, and clears after terminal success
- `Chainworks ForgeTests/Proposal081GraphQLRedactionTests` — Swift decodes typed GraphQL redaction metadata and operator-alert lifecycle/native-delivery fields with accessibility metadata
- migration presence: `control-plane/crates/db/migrations/068_p081_audit_log.sql`, `069_p081_audit_log_checkpoints.sql`, `070_p081_caller_class.sql`, `071_p081_approval_idempotency.sql`, `072_p081_fix_payload_length_check.sql`, `073_p081_approval_idempotency_request_hash.sql`, `074_p081_mcp_command_idempotency.sql`, `075_p081_command_journal_idempotency.sql`

Use when:

- changing the boundary matrix doc/fixture
- editing `control-plane/crates/auth/src/boundary/`, the `auth::resolve` caller-class derivation, `db::repos::audit_log`, or the P081 migrations
- changing Swift approval action idempotency ownership in `RunsHomeView`
- adding or renaming a matrix row, transport, caller class, denial reason code, or rollout phase

Host policy:

- local Rust toolchain and Python 3 required; no UI host, daemon, or network required

Command:

```bash
./scripts/test-gate.sh proposal-081
./scripts/test-gate.sh p081
```

Important:

- `p081` is accepted as an alias
- this gate proves the P081 boundary contract as implemented repository truth: daemon-injected policy wiring, GraphQL/MCP bounded readback, WebSocket close-code enforcement, GraphQL redaction envelopes, Swift redaction/accessibility decoding, approval idempotency, and MCP command idempotency linked to `command_journal`
- the gate fails closed if the fixture is missing any required row, the doc is missing, the auth/db/readback/WebSocket/idempotency/Swift slices fail, or any of the P081 migrations (`064`–`071`) is absent

### `proposal-086|p086|p086-continuation-preflight`

Retained historical gate alias for agent work continuation: migration shape, MCP/artifact JSON Schemas, Rust domain/DB/MCP unit tests for `agents.continue_work`, `agents.continuation_status`, and `agents.continuation_candidates`, the async MCP admission plus terminal-readback contract, GraphQL continuation readback tests, passive SwiftUI readback source guards plus the dedicated P031 Swift readback test slice, durable metrics checks, orphan ACP reap checks, duplicate prompt reconciliation guards, plus a daemon-level MCP -> worker -> live ACP reuse regression. The stable contract lives in [`agent-work-continuation.md`](agent-work-continuation.md); `proposal-086`, `p086`, and `p086-continuation-*` remain gate names only.

Scope:

- `control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql` exists and declares the tables `agent_work_continuations`, `agent_external_side_effect_ledger`, and `supervised_workers_continuation`, the required indexes (`idx_awc_agent_created_at`, `idx_awc_run_status`, `idx_awc_stage`, `idx_awc_recon`, `idx_awc_admission`, `idx_awc_recovery`, `idx_ledger_cont_seq`, `idx_ledger_unresolved`, `idx_swc_heartbeat`, `idx_swc_generation`, `uniq_swc_active_continuation`), and SHA-256 `NOT GLOB` check constraints
- `control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql` declares the durable provider pid/process-group/uid binding used by restart recovery, and `067_p086_continuation_metric_events.sql` declares bounded metric events plus the `idx_p086_metric_run_time` index
- `control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql` exists and declares durable provider process binding fields (`provider_child_pid`, `provider_process_group_id`, `provider_process_uid`) used by restart orphan-reap recovery
- All six MCP schemas under `docs/reference/p086/schemas/mcp/` parse as Draft 2020-12 JSON Schema; `agents.continue_work.request.schema.json` exposes the full continuation context (`run_id`, `stage_execution_id`, `session_generation_id`, `provider_session_id`, `operator_instruction`, `max_turns`, `max_wall_clock_seconds`, `blockers`), and `agents.continue_work.response.schema.json` requires the `outcome` field
- `agents.continue_work.response.schema.json` remains a bounded async admission response (`accepted` / `replay` / `rejected`) and does not reintroduce terminal artifact fields that belong to `agents.continuation_status`, `continuations(runId:)`, and `continuationStatus(agentExecutionId:)`
- `control-plane/crates/engine/src/executor.rs` routes live-handle continuation through ACP `execute(... reuse_existing_session=true ...)`, does not call ACP `start_session` for that path, persists the ACP provider process binding, reconciles duplicate post-`prompt_sent` work only from post-continuation worktree evidence paired with a committed `provider_send` row, records durable continuation metric events for raw counters and KPI rollups (useful progress, validation success, time saved, provider/session budget), and contains the canonical P086 mode-reset prompt guardrails retained by the implementation
- Admission is catalog-gated and side-effect-safe: `examples/agents/agents.yaml` declares `code_writer.continuation_capability`, the workflow catalog model preserves it in frozen snapshots, MCP admission rejects forbidden release/publish/git-push/upload/distribution lanes, engine-owned lead-auto orchestration validates `lead_continuation_decision_v1` and enqueues `ProcessContinuation`, and admission queries unresolved P078 `side_effects` before accepting a continuation
- `lead_auto` may enter through Agent principals only for `trigger_kind=lead_auto`; the handler still requires server-side lead decision artifact identity/hash/target/safety validation before admission
- Startup recovery contains the durable provider process-group reap path and records `orphan_reap_attempted` / `orphan_reap_verified` evidence, signal counts, and TERM/KILL deadlines before releasing stale supervised-worker rows
- GraphQL exposes `continuationStatus`, `continuationCandidates`, `continuations(runId:)`, and `continuationMetricsSummary(runId:)`; the Swift P031 read model and Runs Overview card consume the history/metrics fields, including P086 rates/time-saved/budget rollups, as passive readback only, and `Proposal031ThinGraphQLReadBoundaryTests` runs inside the P086 gate to prove the Swift decode/presentation path
- All nine artifact schemas under `docs/reference/p086/schemas/artifacts/` parse as JSON Schema with `additionalProperties=false`
- Rust tests covering `domain::continuation`, the `agent_work_continuations` repo including metric summaries, the `agents.*` MCP tool surface, auth lead-auto capability, GraphQL continuation history/metrics readback, orphan provider process-group reap, and the daemon-level MCP → continuation worker → live ACP reuse path run green under `cargo test`

Use when:

- Landing or revising the continuation migration, schema artifacts, or Rust admission/repo/MCP/runtime worker surface
- Verifying that the canonical continuation path still reuses a live ACP session instead of falling back to a fresh-session path

Command:

```bash
./scripts/test-gate.sh proposal-086
./scripts/test-gate.sh p086
./scripts/test-gate.sh p086-continuation-preflight
```

Important:

- `p086` and `p086-continuation-preflight` are accepted as aliases for the Phase 0 preflight
- the gate fails closed when any required migration element, SHA-256 constraint, MCP/artifact schema, Rust unit test, passive SwiftUI readback hook, duplicate-send reconciliation guard, or daemon-level live ACP reuse regression is missing or invalid

### `p086-continuation-readback`

Retained Phase 1 readback gate for agent work continuation. Verifies the operator readback fixture `docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json` carries all `operator_readback_v1` decision fields plus the continuation surface (request/response fingerprints, lifecycle status, projection lag, reconciliation status, cancel termination proof, orphan reap outcome, attach receipt, kill-switch last change). The gate rejects placeholder-looking ids/hashes and requires an `evidence_provenance` block tying the fixture to the same-tree daemon/MCP proof instead of accepting synthetic rollout placeholders.

Use when:

- Producing or refreshing operator readback evidence for an internal Phase 1 run
- Verifying readback parity across the `mcp`, `release_receipt`, and `graphql` lanes for an eligible `code_writer` continuation

Command:

```bash
./scripts/test-gate.sh p086-continuation-readback
```

Important:

- fails closed while the fixture is a Phase 0 placeholder; only fixtures with `rollout_contract_status="pass"` and the full operator readback surface satisfy this gate
- distinct from `p086-continuation-preflight` — the two gates address different `rollout_contract_v1` decision surfaces

### `p086-continuation-negative-fixtures`

Retained Phase 2 hold-condition gate for agent work continuation. Verifies that every negative fixture under `docs/evidence/rollout-contract/p086/negative/` exists, is valid JSON, and is no longer a Phase 0 placeholder. Each fixture corresponds to a `rollout_contract_v1` hold condition (admission timeout sweeper, duplicate prompt prevention after `prompt_sent`, idempotency fingerprint conflict, lead decision freshness, malformed hashes, schema lint failures, missing log correlation keys, pre-prompt crash recovery, provider session resurrection ordering, saturation drain, terminal artifact presence, worker panic recovery).

Use when:

- Materializing real SQL/readback/runtime evidence as Phase 2 worker behavior lands
- Proving hold-condition coverage before broader continuation rollout

Command:

```bash
./scripts/test-gate.sh p086-continuation-negative-fixtures
```

Important:

- fails closed if any fixture is missing or still carries `status="placeholder"`
- coupled to continuation worker behavior (provider prompt send, side-effect ledger commit, reconciliation, cancellation lifecycle); fixture evidence requirements are encoded in the fixture set and the stable continuation contract

### `p086-continuation-operator-report`

Retained Phase 1 operator-report gate for agent work continuation. Verifies that the operator readback fixture exposes the subset of operator report fields required by the rollout contract (rollout status/decision/hold conditions/enforcement mode, continuation/run/stage/agent identifiers, mode, trigger kind, lifecycle status, request fingerprint, eligibility, lead decision, confirmation required) and is not a placeholder.

Use when:

- Publishing the operator report after a Phase 1 enablement run
- Verifying the operator-facing surface before expanding continuation enablement

Command:

```bash
./scripts/test-gate.sh p086-continuation-operator-report
```

Important:

- fails closed while the fixture contains a `placeholder` key, has `rollout_contract_status` other than `pass`, or omits any required operator field
- distinct from `p086-continuation-preflight` and `p086-continuation-readback` — operator-report coverage is its own `rollout_contract_v1` decision surface
