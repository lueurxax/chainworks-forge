# Test Gates

Chainworks Forge uses layered test gates instead of one default `xcodebuild test` loop for every change.

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

- refuses to start if `xcodebuild`, `xctest`, `debugserver`, or `Chainworks Forge.app` are already running
- prints the latest known `Chainworks Forge-*.ips` crash log path
- reports a newly created crash log path when a gate fails

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

Command:

```bash
./scripts/test-gate.sh ui-smoke
```

### `proposal-006`

Proposal-specific gate for provider/settings/readiness work.

Scope:

- `ProviderPlatformTests`
- `testProviderSettingsWizardFlowSurface`
- `testProviderSettingsExportSurface`
- `testPilotReadinessRefreshSurface`

Use when:

- changing Proposal 006 implementation or sign-off evidence

Command:

```bash
./scripts/test-gate.sh proposal-006
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

Command:

```bash
./scripts/test-gate.sh full
```

## Recommended Usage

### Normal implementation loop

```bash
./scripts/test-gate.sh fast
```

### UI-heavy work

```bash
./scripts/test-gate.sh ui-smoke
```

### Proposal 006 work

```bash
./scripts/test-gate.sh proposal-006
```

### Before sign-off

```bash
./scripts/test-gate.sh full
```

## Why This Exists

The repository has a real mix of:

- cheap domain/runtime tests
- medium-cost provider/platform slices
- expensive macOS UI automation
- broad baseline tests that catch unrelated crashes outside the active proposal

Running them all on every edit burns time and makes failures harder to interpret. These gates make the cost and purpose of each layer explicit.
