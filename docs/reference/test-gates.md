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

- runtime proof for `GooseProviderConnectionAssistantView`
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

### Before sign-off

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh full"
```

### UI quality proof

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-012"
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
