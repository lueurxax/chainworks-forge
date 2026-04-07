# Proposal 015: Skill Resolution and Runtime Injection Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Dirty (49 modified, 6 untracked)` |
| Audited At | `2026-04-07T10:34:28+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015` remains `Partial` on the current tree and regressed relative to the previous same-proposal audit basis. The proposal-owned skill-resolution and provenance code is still present, but the fresh same-tree non-UI proof attempt now fails at compile time: the targeted macOS slice for `Proposal015Tests` plus `GooseSessionBridgeTests` stopped in `GooseServerTransport.swift` because `RuntimeStreamEventMapper` is no longer in scope. The canonical `proposal-015` gate and `full` regression both remain remote-only, and they are unavailable from this host; the configured approved host was not reachable from this environment. Under the current audit skill, that combination fail-closes the verdict to `Partial` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Fresh same-tree proposal slice does not build | `High` |
| Architecture | `At Risk` | Shared Goose/runtime refactor broke the compile path used by the proposal proof | `High` |
| Product | `At Risk` | The promised execution-time proof cannot currently be reproduced on this tree | `High` |
| UI | `At Risk` | Required `proposal-015` gate is remote-only and unavailable from this host | `High` |
| UX | `Acceptable` | Shell-owned visibility contract still exists in code, but runtime proof is absent | `Medium` |
| Readiness | `Not Ready` | No passing same-tree proposal gate or full regression evidence exists | `High` |

## Proposal Contract

### Scope

- Resolve `external_skill`, `inline_skill`, and `builtin_agent` bindings from YAML.
- Inject resolved skill content and role specialization into execution packets.
- Freeze raw and injected skill provenance into immutable run-start truth.
- Expose persisted execution-time skill truth through shell-owned operator surfaces.

### Locked Decisions

- External skills are Codex bundles rooted at `SKILL.md`.
- `skill_ref` and `skill_role` remain runtime-authoritative.
- Frozen skill truth belongs to immutable `Run` fields plus `RunStartSnapshot`.
- MVP fail-closes instead of truncating executable skill content.
- Visibility extends existing shell-owned report / comparison / artifact lanes.

### Primary User Flows

1. Resolve external, inline, and builtin skills before execution.
2. Inject specialized skill content into Goose-backed execution packets.
3. Freeze raw and injected hashes at run start.
4. Inspect persisted skill truth from report, comparison, readiness, and artifact surfaces.

### UI Commitments

- Catalog and readiness surfaces show resolved skill type / role truth.
- Shell-owned report, comparison, and artifact readers expose persisted execution-time skill truth.
- Invalid skill bindings block launch through preflight.

### UX Commitments

- Operators can inspect execution-time skill truth without leaving existing shell-owned surfaces.
- Missing or invalid skill bindings fail closed before execution starts.

### Acceptance Criteria

- Current bundle shape resolves correctly.
- Resolved skill content reaches execution packets.
- Raw and injected hashes freeze into run-start truth.
- Shell-owned visibility proof exists.
- Proposal-owned same-tree gate passes.

### Test / Evidence Requirements

- Focused tests for resolution, injection, provenance, and visibility.
- App-level UI proof.
- Same-tree successful `full` regression for any successful audit.

### Explicit Exclusions

- No arbitrary runtime truncation of executable skill content.
- No parallel inspection surface outside the existing shell-owned spine.

## Proposal Fidelity / Divergence

### Matches

- Current tree still contains `SkillResolver`, `SkillInjector`, `SkillRoleCustomizer`, `RunStartSnapshot`, and the proposal-owned tests.
- The app still carries the dedicated UI proof hook `testProposal015SkillVisibilityProofSurface`.
- Run-start models still expose `skillContentHashesJSON` and `skillInjectedContentHashesJSON`.

### Divergences

- Fresh same-tree non-UI proof no longer compiles.
- Canonical `proposal-015` gate cannot be executed from this host because it is remote-only.
- Successful roll-up is impossible because `full` is also remote-only here.

### Ambiguities / Evidence Gaps

- The configured approved host could not be reached from this environment, so no fresh remote UI proof was obtainable in this pass.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 External skill bundles resolve from Codex `SKILL.md` roots
- Proposal Source: `§4.1`, `§4.1.1`, `§10 A1`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/ExternalSkillLoader.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: The current tree still targets the Codex `SKILL.md` bundle contract and companion loading model.

### REQ-002 Inline and builtin skills resolve into concrete runtime content
- Proposal Source: `§4.2`, `§4.3`, `§10 A2-A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/BuiltinSkillRegistry.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: Builtin, inline, and external resolution paths are all still present in code and targeted tests.

### REQ-003 Resolved skill content is injected into Goose-backed execution
- Proposal Source: `§5.1-§5.5`, `§10 A1-A4`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-found`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
  - `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' ... -only-testing:'Chainworks ForgeTests/Proposal015Tests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-audit-D8mh9d/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Gap / Note: Injection logic is still present, but fresh same-tree proof did not build because `GooseServerTransport.swift` references missing `RuntimeStreamEventMapper`.

### REQ-004 Preflight blocks invalid skill bindings
- Proposal Source: `§4.4`, `§7`, `§10 A5-A6`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: The preflight and readiness surfaces are still wired in code on the current tree.

### REQ-005 Frozen run-start truth captures raw and injected skill hashes
- Proposal Source: `§6.1-§6.3`, `§10 A7-A8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: `RunStartSnapshot` still persists both raw and injected skill hash maps onto immutable run truth.

### REQ-006 Execution rows and shell-owned readers expose persisted skill truth
- Proposal Source: `§6.3`, `§8.2`, `§10 A8-A10`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
- Gap / Note: The intended shell-owned visibility path remains present in the current implementation.

### REQ-007 End-to-end proposal slice coverage exists for resolution, injection, and provenance
- Proposal Source: `§9 Phases 1-3`, `§10 A12`
- Status: `Partially Implemented`
- Evidence Type: `tests-found`, `tests-run`
- Evidence:
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-audit-D8mh9d/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Gap / Note: Coverage exists, but the fresh same-tree run was cancelled by a build failure before executing tests.

### REQ-008 Canonical proposal-owned gate passes on the same tree
- Proposal Source: `§9 Phase 4`, `§10 A9-A11`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `scripts/test-gate.sh`
  - `./scripts/test-gate.sh proposal-015`
  - output: `error: UI tests are remote-only and may not run on this host.`
  - `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`
  - output: `ssh: Could not resolve hostname smacbook.local`
- Gap / Note: The gate exists, but it could not be executed from this host and no fresh approved-host proof was available.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Shared Goose/runtime refactor broke the compile path used by this proposal
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-003`, `REQ-007`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:293`
  - `Chainworks Forge/Engine/GooseStreamEventMapper.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-audit-D8mh9d/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Why It Matters: `P015` depends on a working Goose-backed execution path for proof. The current tree calls `RuntimeStreamEventMapper.map(...)` even though the available mapper is `GooseStreamEventMapper`, so the proposal slice no longer compiles.
- Recommended Action: Restore a valid stream-mapper reference in the Goose transport stack, then rerun the focused `P015` non-UI slice.

## Product Review

**Summary:** `At Risk`

### PROD-001 The proposal’s execution-time proof is no longer reproducible on the current tree
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-003`, `REQ-007`, `REQ-008`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - focused xcodebuild command above
  - `./scripts/test-gate.sh proposal-015`
- Why It Matters: The proposal promised more than static resolver code. It promised working injected execution plus operator-visible proof. The current tree cannot currently reproduce that end-to-end contract.
- Recommended Action: Fix the shared compile break first, then re-establish a fresh approved-host UI proof for `proposal-015`.

## UI Review

**Summary:** `At Risk`

### UI-001 Required `proposal-015` UI proof remains remote-only and unreachable from this audit host
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `runtime`
- Evidence:
  - `./scripts/test-gate.sh proposal-015`
  - `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`
- Why It Matters: The proposal’s shell-owned visibility promise still depends on a dedicated UI proof lane. That lane was not runnable in this pass.
- Recommended Action: Restore approved-host reachability or run the audit from an approved host before claiming UI proof.

## UX Review

**Summary:** `Acceptable`

### UX-001 Shell-owned visibility contract is still coherent in code, but runtime confidence is reduced
- Severity: `Minor`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-006`, `REQ-008`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
- Why It Matters: The proposal’s UX direction is still represented in code, so this is not a design-collapse problem. The gap is execution proof and readiness.
- Recommended Action: Revalidate the same shell-owned surfaces once the build and remote proof lane are restored.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Fresh same-tree non-UI proof is red
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-003`, `REQ-007`
- Evidence Type: `tests-run`
- Evidence:
  - focused xcodebuild command above
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-audit-D8mh9d/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Why It Matters: Even the narrow local slice needed to establish fresh proposal proof does not currently build.
- Recommended Action: Fix the shared Goose compile break before running any further audit proof.

### READY-002 Same-tree successful roll-up is blocked because `full` is unavailable from this host
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `runtime`
- Evidence:
  - `./scripts/test-gate.sh full`
  - output: `error: UI tests are remote-only and may not run on this host.`
- Why It Matters: The audit skill forbids `Implemented`, `Ready`, and `Ready with Risks` without successful same-tree `full` regression.
- Recommended Action: Run `full` on an approved remote UI host after restoring approved-host connectivity and fixing the current build break.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Fail` | Focused macOS xcodebuild slice failed with `Cannot find 'RuntimeStreamEventMapper' in scope` |
| Core user flow runtime-validated | `Fail` | Execution/injection slice did not build; UI proof lane unavailable |
| Empty/loading/error states covered | `Partial` | Static surfaces exist, runtime not revalidated |
| Accessibility risk acceptable | `Not Checked` | No fresh UI proof in this pass |
| Localization risk acceptable | `Not Checked` | Out of scope for this pass |
| Critical tests executed | `Partial` | Focused xcodebuild slice executed but failed during build; remote gate unavailable |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | `full` is remote-only from this host and no approved-host run was available |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not proposal-critical in this pass |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/015-skill-resolution-and-runtime-injection.md`
- `rg -n "SkillResolver|SkillInjector|SkillRoleCustomizer|RunStartSnapshot|skillInjectedContentHashesJSON|skillContentHashesJSON|testProposal015SkillVisibilityProofSurface|Proposal015Tests" ...`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$DERIVED_DATA" test -only-testing:'Chainworks ForgeTests/Proposal015Tests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'`
- `./scripts/test-gate.sh proposal-015`
- `./scripts/test-gate.sh full`
- `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`

## Recommended Next Actions

1. Fix the `RuntimeStreamEventMapper` / `GooseStreamEventMapper` mismatch in the shared Goose transport compile path.
2. Re-run the focused `P015` non-UI slice locally to restore fresh same-tree proposal proof.
3. Restore approved-host reachability and rerun `./scripts/test-gate.sh proposal-015`.
4. Only after the above, run same-tree `./scripts/test-gate.sh full` on an approved host for a successful roll-up.
