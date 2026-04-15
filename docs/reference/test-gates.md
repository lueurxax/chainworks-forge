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
- start run sheet
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
- `proposal-033` is the repo-owned proof lane for [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](../reference/033-remove-goose-from-canonical-transport-and-simplify-runtime.md)

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
