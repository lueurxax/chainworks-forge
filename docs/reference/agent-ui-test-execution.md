# Agent UI Test Execution

Operational playbook for how agents should validate macOS UI behavior in Chainworks Forge.

This document is intentionally execution-oriented. It defines:

- which UI validation mode to use
- when local execution is forbidden or inappropriate
- how to run focused XCUITest safely
- how to run app-launched repo-backed proof flows
- what evidence actually counts for proposal sign-off

Use this together with [test-gates.md](test-gates.md) and [test-suite-architecture.md](test-suite-architecture.md). The gate doc answers "which layer should I run"; this document answers "how can an agent run UI proof at all without causing operator pain or collecting fake evidence."

## Single Source Of Truth

For both Codex and Claude Code, this document is the canonical execution contract for any macOS UI or app-launched proof.

- Do not invent alternate SSH targets.
- Do not omit the SSH user.
- Do not treat local UI execution as an acceptable fallback.
- Do not substitute a raw local `xcodebuild test` loop for the documented remote flow.

## Canonical Remote Identity

For remote UI work, the canonical target is:

```text
test@SMacBook.local
```

This is not optional shorthand.

- Do not assume that bare `SMacBook.local` will select the correct user.
- Do not write docs or prompts that only mention the host alias when the command is meant to be executed over SSH.
- If the approved host list changes, the host alias may change, but the documentation must still name the actual SSH login target explicitly.

Current approved remote UI SSH target:

- `test@SMacBook.local`

Canonical remote workspace:

- `/Users/test/chainworks-remote`

The repository gate policy still reasons about approved host names (`SMacBook.local`, `SMacBook`), but agents invoking remote commands must use the explicit SSH user.

## Canonical Remote-Only Rule For Agents

Both Codex and Claude Code should follow the same rule set:

1. Do not launch the app or UI tests on the local laptop when the operator forbids it.
2. Treat remote UI validation as the default path for any app/UI execution.
3. Use `test@SMacBook.local` as the canonical remote SSH target.
4. Prefer repository gate commands over ad hoc `xcodebuild` loops.
5. Pull back `xcresult`, exported evidence packs, or result JSON after the run so the proof is inspectable locally.

### Non-Negotiable Agent Rule

If Codex or Claude Code needs to prove UI behavior, the default command form is:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh <gate>"
```

Anything else is an exception that must be justified by a specific infrastructure limitation.

## Core Rule

Agents must treat UI execution as a host policy decision, not just a test command.

- If the operator says "do not launch the app on this laptop", do not launch it locally.
- If the local host is already unstable or polluted with stale `xcodebuild` / `xctest` / app processes, stop and clean up before attempting any new UI path.
- If the target proof requires a real app session and the local host is disallowed, move the proof to a remote macOS host instead of quietly violating the instruction.

This repository already has both kinds of UI validation:

- small surface proofs through XCTest UI automation
- proposal-level app-launched dogfood harnesses that run inside the app process

They solve different problems.

## Canonical UI Validation Modes

### 1. Preview Review

Use preview review for:

- layout quality
- hierarchy
- progressive disclosure
- collapsed navigation behavior
- empty/loading/error composition
- accessibility surface shape before interaction

Preferred tools:

- Xcode MCP preview/render path
- SwiftUI `#Preview` coverage in the view source

Preview review is design validation. It does not prove interaction correctness.

### 2. Focused XCUITest Proof

Use focused XCUITest for:

- tab reachability
- start-run sheet reachability
- approval gate visibility
- run progress visibility
- provider/settings/wizard reachability
- direct operator-shell regressions

Preferred entrypoints:

- `./scripts/test-gate.sh ui-smoke`
- targeted `xcodebuild test -only-testing:...` commands on a macOS host with working Xcode

Execution rule:

- focused UI proof should go through the repository gate runner unless there is a specific diagnostic reason not to
- raw `xcodebuild -testPlan ...` runs are diagnostic-only until they are shown to execute a non-zero intended test set on the current toolchain

Primary files:

- [Chainworks_ForgeUITests.swift](../../Chainworks%20ForgeUITests/Chainworks_ForgeUITests.swift)
- [StartRunScreen.swift](../../Chainworks%20ForgeUITests/StartRunScreen.swift)
- [RunProgressScreen.swift](../../Chainworks%20ForgeUITests/RunProgressScreen.swift)
- [IdeasScreen.swift](../../Chainworks%20ForgeUITests/IdeasScreen.swift)
- [AppScreen.swift](../../Chainworks%20ForgeUITests/AppScreen.swift)

Focused XCUITest is the correct layer for operator-shell reachability, not for proving a full repo-backed delivery contract.

### 3. App-Launched Dogfood Proof

Use app-launched proof when the contract is larger than a surface smoke test:

- repo-backed delivery
- worktree-backed live execution
- manual release gate
- exported evidence pack from a real app session
- proposal sign-off flows such as Proposal 007

Canonical implementation:

- [Proposal007DogfoodHarness.swift](../../Chainworks%20Forge/Engine/Proposal007DogfoodHarness.swift)

This path runs inside the app process, drives the run to terminal state, exports a real evidence pack, and persists a result JSON. It is the preferred fallback when `Chainworks ForgeUITests-Runner.app` is signing-broken, Gatekeeper-blocked, or too fragile for proposal-level truth.

## Host Selection

### Local Laptop

Allowed uses:

- preview review
- compile-only checks
- read-only diagnostics
- documentation work

Disallowed or discouraged uses:

- any UI or app launch after the operator explicitly forbids it
- repeated `xcodebuild test` loops that spawn the app and interfere with active development
- proposal sign-off proof when the host already showed churn, hanging runners, or unexpected relaunches

Current repository policy is stricter than a recommendation:

- UI tests are remote-only by default.
- The UI test target enforces an approved-host check.
- `./scripts/test-gate.sh` refuses `ui-smoke`, `proposal-006`, and `full` on non-approved hosts.

Default approved hosts:

- `SMacBook.local`
- `SMacBook`

Canonical SSH login:

- `test@SMacBook.local`

If infrastructure changes, update both:

- [Chainworks_ForgeUITests.swift](../../Chainworks%20ForgeUITests/Chainworks_ForgeUITests.swift)
- [scripts/test-gate.sh](../../scripts/test-gate.sh)

### Remote macOS Host

Use a remote Mac when:

- the operator requires that app/UI runs not happen on the current laptop
- proposal sign-off needs fresh exported evidence
- the local host has a broken runner/signing situation
- an app-launched dogfood flow is required

Remote host requirements:

- full Xcode installed and selected
- working `xcodebuild`
- writable repository checkout
- user session able to launch the app
- enough disk space for DerivedData, run storage, worktrees, and exported evidence packs

The current approved remote host policy is repository-enforced, not just operator lore.

## SSH Prerequisites

Remote UI testing is not considered ready until passwordless SSH works.

Minimum requirement:

```bash
ssh -o BatchMode=yes test@SMacBook.local 'printf ok'
```

Expected result:

```text
ok
```

If that fails, fix SSH before attempting UI gates.

Preferred setup:

```bash
ssh-copy-id -i ~/.ssh/id_ed25519.pub test@SMacBook.local
```

If `ssh-copy-id` is unavailable or fails, the fallback is:

1. create `~/.ssh` on the remote host,
2. append the local public key to `~/.ssh/authorized_keys`,
3. set permissions:
   - `chmod 700 ~/.ssh`
   - `chmod 600 ~/.ssh/authorized_keys`

Agents should treat password-prompt SSH as infrastructure-not-ready for routine UI proof.

Quick verification sequence:

```bash
ssh -o BatchMode=yes test@SMacBook.local 'printf ok'
ssh test@SMacBook.local 'xcodebuild -version && xcode-select -p'
```

## Canonical Remote Workspace

When an agent needs a remote checkout, use a stable workspace path on the approved host.

Canonical default:

```text
/Users/test/chainworks-remote
```

If another remote checkout is intentionally used, the command should state it explicitly.

## Canonical Remote Workflow

For both Codex and Claude Code, the recommended remote flow is:

1. verify SSH key login,
2. verify remote Xcode,
3. sync or update the repository on the remote host,
4. run the correct gate on the remote host,
5. pull back proof artifacts.

### 1. Verify SSH

```bash
ssh -o BatchMode=yes test@SMacBook.local 'hostname && pwd'
```

### 2. Verify Xcode

```bash
ssh test@SMacBook.local 'xcodebuild -version && xcode-select -p'
```

### 3. Prepare the remote checkout

If the remote checkout already exists:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && git status --short"
```

Recommended sync when the remote checkout tracks the same repository:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && git fetch --all --prune && git status --short"
```

If the agent needs to copy the current local tree:

```bash
tar czf - -C "/Users/user/Documents/Chainworks Forge" --exclude .git --exclude .codex . \
  | ssh test@SMacBook.local "mkdir -p /Users/test/chainworks-remote && tar xzf - -C /Users/test/chainworks-remote"
```

### 4. Run the gate remotely

Canonical examples:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh build"
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh fast"
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"
```

Do not replace these with bare `ssh SMacBook.local ...`.
Do not assume the shell default user on the remote host is correct.

### 5. Pull back proof artifacts

Gate runner artifacts live under:

```text
/tmp/chainworks-test-gates
```

Example copy-back:

```bash
scp "test@SMacBook.local:/tmp/chainworks-test-gates/ui-smoke-*.xcresult" ./
```

Canonical copy-back examples:

```bash
scp "test@SMacBook.local:/tmp/chainworks-test-gates/build-*.log" ./
scp "test@SMacBook.local:/tmp/chainworks-test-gates/fast-*.xcresult" ./
scp "test@SMacBook.local:/tmp/chainworks-test-gates/ui-smoke-*.xcresult" ./
scp -r "test@SMacBook.local:/Users/test/Desktop/evidence-pack-*" ./
```

For proposal-level app-launched proof, also copy back:

- exported evidence packs,
- result JSON,
- any run-storage artifact directories required by the proof contract.

## Before Any UI Run

Agents should verify the host is clean before starting UI work.

Minimum checks:

- no active `xcodebuild`
- no active `xctest` / `XCTest`
- no active `debugserver`
- no active `Chainworks Forge.app` left over from prior runs

The repository gate runner enforces this for gate-based execution:

```bash
./scripts/test-gate.sh ui-smoke
```

It refuses to start when stale test/app processes are already running and reports the latest crash log path before and after failures.
It also refuses UI-carrying gates on non-approved hosts.

When invoked remotely, the canonical form is:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"
```

If the gate runner refuses to start because the host is busy, agents should clean the remote host first instead of switching back to local execution.

## Required Environment Controls

### Disable Eager Bootstrap During UI Automation

Use this when focused XCUITest should not trigger eager bootstrap behavior:

```text
CHAINWORKS_UI_TEST_DISABLE_EAGER_BOOTSTRAP=1
```

Relevant wiring:

- [Chainworks_ForgeApp.swift](../../Chainworks%20Forge/Chainworks_ForgeApp.swift)
- [Chainworks_ForgeUITests.swift](../../Chainworks%20ForgeUITests/Chainworks_ForgeUITests.swift)

This is the canonical control for UI automation hosts. Agents should prefer it over ad hoc code edits or test-only runtime hacks.

### Proposal 007 App-Launched Dogfood Harness

Enable the app-driven repo-backed delivery proof path with:

```text
CHAINWORKS_P007_DOGFOOD_AUTORUN=1
```

Important companion variables:

- `CHAINWORKS_IN_MEMORY_STORE=1`
- `CHAINWORKS_GOOSE_FIXTURE_MODE=full_mvp_success`
- `CHAINWORKS_DELIVERY_PROOF_MODE=happy_path` or `non_happy_path`
- `CHAINWORKS_UI_TEST_SEED_IDEA_TITLE=...`
- `CHAINWORKS_UI_TEST_SEED_IDEA_BODY=...`
- `CHAINWORKS_UI_TEST_SEED_IDEA_WORKSPACE_ROOT=...`
- `CHAINWORKS_RUN_STORAGE_BASE_PATH=...`
- optional `CHAINWORKS_DOGFOOD_WORKTREE_BASE_PATH=...`
- optional `CHAINWORKS_DOGFOOD_EXPORT_BASE_PATH=...`
- optional `CHAINWORKS_DOGFOOD_RESULT_PATH=...`

Behavior:

- creates or updates the seeded idea
- compiles a real repo-backed run
- resolves provider bindings
- validates delivery preflight
- provisions a real worktree
- auto-resolves approvals
- waits for terminal state
- exports an evidence pack
- persists a `Proposal007DogfoodResult`
- terminates the app

If `CHAINWORKS_DOGFOOD_EXPORT_BASE_PATH` is omitted, export defaults to the current user's Desktop. This is intentional and is part of the proposal sign-off contract.

## Canonical Command Patterns

### Preview-Oriented Review

Use Xcode MCP preview/render. Do not treat preview success as interaction proof.

### Focused XCUITest

For a single test:

```bash
xcodebuild \
  -project "Chainworks Forge.xcodeproj" \
  -scheme "Chainworks Forge" \
  -destination "platform=macOS" \
  -only-testing:"Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI" \
  test
```

For the standard shell smoke slice:

```bash
./scripts/test-gate.sh ui-smoke
```

Remote canonical form:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"
```

For Codex and Claude Code, this is the normal proving path for UI smoke.

### Proposal 007 App-Launched Happy Path

Minimal shape:

```bash
CHAINWORKS_P007_DOGFOOD_AUTORUN=1 \
CHAINWORKS_IN_MEMORY_STORE=1 \
CHAINWORKS_GOOSE_FIXTURE_MODE=full_mvp_success \
CHAINWORKS_DELIVERY_PROOF_MODE=happy_path \
CHAINWORKS_UI_TEST_SEED_IDEA_TITLE="P007 Happy Proof" \
CHAINWORKS_UI_TEST_SEED_IDEA_BODY="Remote app-launched happy-path proof." \
CHAINWORKS_UI_TEST_SEED_IDEA_WORKSPACE_ROOT="/absolute/path/to/Chainworks Forge" \
CHAINWORKS_RUN_STORAGE_BASE_PATH="/absolute/path/to/proof/storage" \
CHAINWORKS_DOGFOOD_RESULT_PATH="/absolute/path/to/result.json" \
open "Chainworks Forge.xcodeproj"
```

### Proposal 007 App-Launched Non-Happy Path

Minimal shape:

```bash
CHAINWORKS_P007_DOGFOOD_AUTORUN=1 \
CHAINWORKS_IN_MEMORY_STORE=1 \
CHAINWORKS_GOOSE_FIXTURE_MODE=full_mvp_success \
CHAINWORKS_DELIVERY_PROOF_MODE=non_happy_path \
CHAINWORKS_UI_TEST_SEED_IDEA_TITLE="P007 Non-Happy Proof" \
CHAINWORKS_UI_TEST_SEED_IDEA_BODY="Remote app-launched non-happy-path proof." \
CHAINWORKS_UI_TEST_SEED_IDEA_WORKSPACE_ROOT="/absolute/path/to/Chainworks Forge" \
CHAINWORKS_RUN_STORAGE_BASE_PATH="/absolute/path/to/proof/storage" \
CHAINWORKS_DOGFOOD_RESULT_PATH="/absolute/path/to/result.json" \
open "Chainworks Forge.xcodeproj"
```

These commands are patterns, not copy-paste canon. Agents should place proof output under a dedicated directory on the remote host and keep happy-path and non-happy-path runs separate.

Recommended remote shape:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && \
CHAINWORKS_P007_DOGFOOD_AUTORUN=1 \
CHAINWORKS_IN_MEMORY_STORE=1 \
CHAINWORKS_GOOSE_FIXTURE_MODE=full_mvp_success \
CHAINWORKS_DELIVERY_PROOF_MODE=happy_path \
CHAINWORKS_UI_TEST_SEED_IDEA_TITLE='P007 Happy Proof' \
CHAINWORKS_UI_TEST_SEED_IDEA_BODY='Remote app-launched happy-path proof.' \
CHAINWORKS_UI_TEST_SEED_IDEA_WORKSPACE_ROOT='/Users/test/chainworks-remote' \
CHAINWORKS_RUN_STORAGE_BASE_PATH='/Users/test/p007-proof/happy/storage' \
CHAINWORKS_DOGFOOD_RESULT_PATH='/Users/test/p007-proof/happy/result.json' \
open 'Chainworks Forge.xcodeproj'"
```

## What Counts As Valid Evidence

### Surface-Level UI Evidence

Valid for shell/operator regressions:

- green focused XCUITest slice
- direct-surface seeded UI proofs
- app-rendered screenshots or preview render for purely visual review

This level is enough for navigation, reachability, and visible-state regressions.

### Proposal-Level Sign-Off Evidence

Required for repo-backed delivery work:

- app-launched or UI-launched real run
- fresh run storage artifacts from that same session
- exported evidence pack from that same session
- terminal state truth that matches the scenario
- happy-path and non-happy-path proof when the proposal requires both

For Proposal 007 specifically, direct-surface smoke tests are useful but insufficient on their own.

## Proposal 007 Acceptance Evidence

For `007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding`, agents should collect all of the following:

- one happy-path app-launched repo-backed run
- one non-happy-path app-launched repo-backed run
- exported evidence packs from both runs
- result JSON from both runs
- real delivery artifacts in run storage

Expected happy-path delivery artifacts:

- `release_manifest`
- `git_push_receipt`
- `release_bundle_manifest`
- `connect_upload_receipt`
- `delivery_receipt`

Expected non-happy-path delivery artifacts:

- `release_manifest`
- `git_push_receipt`
- `delivery_receipt`
- no `connect_upload_receipt` when the publish stage is the intended failure point

Agents should also verify real worktree truth when the scenario claims code changes were made. A manifest alone is not enough.

## Known Infrastructure Limitations

### Full-Scheme `xcodebuild test` Is Not the Same as Focused UI Proof

This repository has already observed cases where:

- full-scheme test attempts still build the UI target even when `-skip-testing:'Chainworks ForgeUITests'` is supplied
- Swift Testing filters can result in effectively zero executed unit tests if the invocation is malformed
- UI target signing issues fail the whole command before the relevant slice even runs

Implication:

- do not over-interpret one broad `xcodebuild test` failure as proof that the proposal slice itself is broken
- use focused unit/UI slices and app-launched proof for proposal-level truth

### `UITests-Runner` Policy or Signing Failure

Symptoms:

- Gatekeeper rejection
- `code has no resources but signature indicates they must be present`
- missing `Mac Development` signing identity
- UI runner builds or launches unreliably even though the app itself builds

Preferred response:

1. do not pretend the XCUITest path is valid
2. keep focused UI smoke limited to the host capability that actually works
3. switch proposal sign-off proof to an app-launched harness when available

## Agent Checklist

Before claiming UI proof is complete, Codex and Claude Code should be able to answer yes to all of these:

- Was the run executed on `test@SMacBook.local`, not locally?
- Was the gate started through `./scripts/test-gate.sh` unless the step was explicitly diagnostic-only?
- Was the remote checkout path stated explicitly?
- Were `xcresult`, result JSON, or evidence-pack artifacts copied back or otherwise recorded?
- If the proof is proposal-level, does it include real app-launched or repo-backed artifacts rather than only a seeded direct-surface test?

### Interaction Hangs in XCUITest

Typical cases already seen in this repository:

- collapsed sidebar hides the real owner path
- empty detail state after creating an idea
- `Picker`-based workflow selection times out
- button remains off-screen in run progress

Preferred response:

- fix the UI or accessibility owner path first
- use preview to inspect composition
- keep helper queries narrow and explicit
- avoid broad `waitForExistence` on ambiguous queries

### False Artifact Truth

Example:

- a `changed_files_manifest` artifact claims a file was changed
- but the actual repo/worktree has no diff

Preferred response:

- trust the real worktree and receipts over summary artifacts
- inspect `git status` in the provisioned worktree
- treat this as a runtime truth bug, not a UI bug

## Recommended Execution Order For Agents

When changing UI-heavy behavior, agents should prefer this order:

1. Preview review for composition and discoverability.
2. Focused XCUITest for reachability and interaction.
3. App-launched dogfood proof for proposal contracts that include real repo-backed side effects.

For Proposal 007-like delivery slices, the final proof should include:

- one app-launched happy-path run
- one app-launched non-happy-path run
- exported evidence packs from both runs
- confirmation that delivery artifacts exist in run storage, not only in exported summaries

## Related Docs

- [test-gates.md](test-gates.md)
- [test-suite-architecture.md](test-suite-architecture.md)
- [operator-experience.md](operator-experience.md)
- [goose-provider-remediation.md](goose-provider-remediation.md)
- [live-workflow-map.md](live-workflow-map.md)
