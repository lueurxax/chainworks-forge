# Proposal 015: Skill Resolution and Runtime Injection Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Clean` |
| Audited At | `2026-04-07T11:30:43+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015` no longer has the old shared compile blocker. The fresh same-tree non-UI proposal slice is green locally, and the synced approved-host non-UI lane is also green. The remaining blockers are now proof-lane and readiness blockers: the canonical `proposal-015` split gate aborts before the UI half with `signing_args[@]: unbound variable`, the direct approved-host UI proof lane fails during codesign of `libXCTestBundleInject.dylib`, and the synced approved-host `full` gate is likewise unavailable because the same shell-script bug aborts before a usable regression run. Under the current audit skill, that fail-closes the roll-up to `Partial` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Approved-host UI proof is still not executable on the synced tree | `High` |
| Architecture | `Acceptable` | Core skill-resolution and provenance paths are green again | `High` |
| Product | `At Risk` | The promised execution-time proof cannot currently be reproduced end to end | `High` |
| UI | `At Risk` | Canonical UI proof lane is broken by gate/codesign issues | `High` |
| UX | `Acceptable` | Shell-owned visibility surfaces remain coherent in code and proof fixtures | `Medium` |
| Readiness | `Not Ready` | Same-tree full regression is unavailable and UI proof is red | `High` |

## Proposal Contract

### Scope

- Resolve `external_skill`, `inline_skill`, and builtin skill references from YAML.
- Inject resolved skill content and role specialization into runtime execution packets.
- Freeze raw and injected skill provenance into immutable run-start truth.
- Surface persisted execution-time skill truth through existing shell-owned operator readers.

### Locked Decisions

- External skills are Codex bundles rooted at `SKILL.md`.
- `skill_ref` and `skill_role` remain runtime-authoritative.
- Frozen skill truth belongs to immutable `Run` fields plus `RunStartSnapshot`.
- MVP fail-closes instead of truncating executable skill content.
- Visibility extends existing shell-owned report / comparison / artifact lanes.

### Primary User Flows

1. Resolve builtin, inline, and external skills before execution.
2. Inject specialized skill content into Goose-backed execution packets.
3. Freeze raw and injected skill hashes at run start.
4. Inspect persisted execution-time skill truth from shell-owned readers.

### UI Commitments

- Readiness and report surfaces expose resolved skill type / role truth.
- Shell-owned report, comparison, and artifact readers expose persisted execution-time skill truth.
- Invalid skill bindings block launch through preflight.

### UX Commitments

- Operators can inspect execution-time skill truth without leaving the existing shell-owned spine.
- Missing or invalid skill bindings fail closed before execution starts.

### Acceptance Criteria

- Current external bundle shape resolves correctly.
- Resolved content reaches runtime execution packets.
- Raw and injected hashes freeze into run-start truth.
- Shell-owned visibility proof exists.
- Proposal-owned same-tree gate passes.

### Test / Evidence Requirements

- Focused proof for resolution, injection, provenance, and shell-owned visibility.
- Approved-host UI proof surface.
- Passing same-tree `full` regression for any successful audit.

### Explicit Exclusions

- No runtime truncation of executable skill content.
- No parallel inspection surface outside the existing shell-owned spine.

## Proposal Fidelity / Divergence

### Matches

- `SkillResolver`, `ExternalSkillLoader`, `SkillInjector`, `SkillRoleCustomizer`, and `RunStartSnapshot` remain present and wired.
- `Proposal015Tests` passed locally on the current tree.
- The proposal-owned UI proof hook `testProposal015SkillVisibilityProofSurface` still exists.

### Divergences

- The canonical `proposal-015` split gate is not healthy on the synced approved-host tree: it stops after the non-UI half with `signing_args[@]: unbound variable`.
- The direct approved-host UI proof invocation fails before execution because codesign fails on `libXCTestBundleInject.dylib`.
- The synced approved-host `full` gate is unavailable for the same shell-script reason, so no successful roll-up is possible.

### Ambiguities / Evidence Gaps

- No fresh same-tree full regression evidence exists because the canonical `full` gate aborts before launching `xcodebuild`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Skill references resolve from current builtin, inline, and external contracts
- Proposal Source: `§4.1-§4.3`, `§10 A1-A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/ExternalSkillLoader.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - local result bundle: `/tmp/p015-audit.v8kP60/Logs/Test/Test-Chainworks Forge-2026.04.07_11-21-23-+0300.xcresult`
- Gap / Note: The current tree resolves the Codex `SKILL.md` bundle shape and the focused proposal slice passed `13/13`.

### REQ-002 Resolved skill content is injected into runtime execution packets
- Proposal Source: `§5.1-§5.5`, `§10 A1-A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - local result bundle: `/tmp/p015-audit.v8kP60/Logs/Test/Test-Chainworks Forge-2026.04.07_11-21-23-+0300.xcresult`
- Gap / Note: The old Goose compile break is closed on the current tree and the focused packet/injection proof now passes.

### REQ-003 Raw and injected skill hashes freeze into immutable run-start truth
- Proposal Source: `§6.1-§6.3`, `§10 A7-A8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - local result bundle: `/tmp/p015-audit.v8kP60/Logs/Test/Test-Chainworks Forge-2026.04.07_11-21-23-+0300.xcresult`
- Gap / Note: The run-start truth path remains green in the fresh same-tree slice.

### REQ-004 Shell-owned readers expose persisted execution-time skill truth
- Proposal Source: `§6.3`, `§8.2`, `§10 A8-A10`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - synced approved-host result bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-112541.xcresult`
- Gap / Note: The proof fixtures and shell-owned visibility contract remain present and the non-UI approved-host slice passes.

### REQ-005 Approved-host UI proof surface executes successfully on the same tree
- Proposal Source: `§9 Phase 4`, `§10 A9-A11`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1219`
  - direct approved-host UI invocation:
    `xcodebuild -scheme "Chainworks Forge" -destination "platform=macOS" -derivedDataPath /tmp/p015-ui.BVyasV test -parallel-testing-enabled NO -maximum-parallel-testing-workers 1 -only-testing:"Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal015SkillVisibilityProofSurface"`
  - approved-host result bundle: `/tmp/p015-ui.BVyasV/Logs/Test/Test-Chainworks Forge-2026.04.07_11-27-12-+0300.xcresult`
- Gap / Note: The proof surface exists, but the fresh approved-host run fails during codesign with `libXCTestBundleInject.dylib: errSecInternalComponent`.

### REQ-006 Canonical proposal-owned proof lane passes on the same tree
- Proposal Source: `§9 Phase 4`, `§10 A9-A12`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `scripts/test-gate.sh:724-734`
  - synced approved-host invocation: `./scripts/test-gate.sh proposal-015`
  - synced approved-host result bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-112541.xcresult`
  - gate output: `./scripts/test-gate.sh: line 734: signing_args[@]: unbound variable`
- Gap / Note: The canonical split gate only completed the non-UI half. It aborted before the UI half, so the proposal-owned proof lane is not passing.

## Architecture Review

**Summary:** `Acceptable`

No fresh architecture finding reopened the proposal. The core skill-resolution, injection, and run-start provenance path is healthy again on the current tree.

## Product Review

**Summary:** `At Risk`

### PROD-001 The proposal’s end-to-end proof promise is still unmet on the synced tree
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - synced approved-host `proposal-015` invocation above
  - approved-host direct UI invocation above
- Why It Matters: `P015` promised more than static code presence. It promised reproducible execution-time proof for shell-owned visibility. That promise is still not satisfied on the current same-tree basis.
- Recommended Action: Fix the split-gate shell bug and restore a working approved-host UI proof lane before claiming the proposal is implemented.

## UI Review

**Summary:** `At Risk`

### UI-001 Approved-host UI proof is blocked in codesign before the proof surface runs
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host result bundle: `/tmp/p015-ui.BVyasV/Logs/Test/Test-Chainworks Forge-2026.04.07_11-27-12-+0300.xcresult`
  - failure text: `libXCTestBundleInject.dylib: errSecInternalComponent`
- Why It Matters: The UI proof lane is part of the proposal’s acceptance contract. At the moment, that lane fails before it can validate the shell-owned surface.
- Recommended Action: Fix the approved-host codesign/test-runner configuration, then rerun `testProposal015SkillVisibilityProofSurface`.

## UX Review

**Summary:** `Acceptable`

No fresh UX contradiction surfaced. The shell-owned report/comparison/artifact visibility model remains consistent with the proposal; the current gap is proof execution, not interaction design.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Canonical `proposal-015` gate is script-broken on the synced approved-host tree
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: `runtime`
- Evidence:
  - `scripts/test-gate.sh:724-734`
  - synced approved-host output: `./scripts/test-gate.sh: line 734: signing_args[@]: unbound variable`
- Why It Matters: Even the canonical proof route is not currently reproducible. That blocks repeatable sign-off for this proposal.
- Recommended Action: Initialize or safely expand `signing_args` in the split gate before the UI branch is invoked.

### READY-002 Same-tree full regression is unavailable, so a successful audit roll-up is forbidden
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: `runtime`
- Evidence:
  - synced approved-host invocation: `./scripts/test-gate.sh full`
  - output: `./scripts/test-gate.sh: line 563: signing_args[@]: unbound variable`
  - local script reference: `scripts/test-gate.sh:562-568`
- Why It Matters: The updated audit skill requires passing same-tree full regression for any successful verdict. That evidence is unavailable on the current synced tree.
- Recommended Action: Fix the canonical `full` gate first; then rerun full regression on the same synced tree before attempting another green audit.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Local focused `xcodebuild test` built and passed. |
| Core user flow runtime-validated | `Partial` | Local non-UI slice and synced approved-host non-UI slice passed; approved-host UI proof is still red. |
| Empty/loading/error states covered | `Partial` | Covered indirectly by proposal-owned tests and fixtures, but no passing UI proof lane. |
| Accessibility risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Localization risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Critical tests executed | `Pass` | Local `Proposal015Tests` passed `13/13`; synced approved-host non-UI lane passed `13/13`. |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | Synced approved-host `full` gate aborts with `signing_args[@]: unbound variable`. |
| Privacy/permissions/entitlements reviewed | `Partial` | UI proof failure is currently in codesign/test-runner setup, not user-facing permission flow. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py '/Users/user/Documents/Chainworks Forge/docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p015-audit.v8kP60 test -only-testing:'Chainworks ForgeTests/Proposal015Tests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'`
- `rsync -az --delete --exclude='.git' --exclude='DerivedData' --exclude='.build' --exclude='*.xcresult' '/Users/user/Documents/Chainworks Forge/' 'test@SMacBook.local:/Users/test/chainworks-remote/'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && xcodebuild ... -only-testing:"Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal015SkillVisibilityProofSurface"'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh full'`

## Recommended Next Actions

1. Fix `scripts/test-gate.sh` so empty `signing_args` expansions do not abort `proposal-015` or `full`.
2. Repair the approved-host UI codesign path for `libXCTestBundleInject.dylib`.
3. Rerun synced approved-host `proposal-015` and then synced approved-host `full`.
