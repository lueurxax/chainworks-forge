# Proposal 015: Skill Resolution and Runtime Injection Multi-Lens Audit R7

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `d8ccf4b` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-07T12:13:50+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015` is no longer blocked by the old `signing_args[@]: unbound variable` shell bug, and approved-host access is available again. The fresh same-tree canonical gate is still red, but now on a stricter and newer basis: after syncing the current dirty tree to `SMacBook.local`, `./scripts/test-gate.sh proposal-015` reached the non-UI half and then failed because the shared `Chainworks ForgeTests` target compile-fails in `Proposal026Tests.swift`. That means the gate never reaches the Proposal 015 assertion set or the UI proof half. The core P015 implementation remains present in code, but the proposal-owned proof lane is not currently executable on the same tree, so the audit fail-closes to `Partial` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Canonical same-tree proof is blocked before Proposal 015 tests can execute | `High` |
| Architecture | `Acceptable` | Resolver / injector / run-start provenance paths still exist coherently in code | `High` |
| Product | `At Risk` | The proposal’s proof promise cannot currently be reproduced end to end | `High` |
| UI | `At Risk` | The required UI proof half is never reached on the synced approved-host tree | `High` |
| UX | `Acceptable` | Shell-owned visibility surfaces remain coherent in the implementation | `Medium` |
| Readiness | `Not Ready` | Same-tree canonical gate is red before any successful roll-up is even eligible | `High` |

## Proposal Contract

### Scope

- Resolve builtin, inline, and external skills from YAML/runtime bindings.
- Inject resolved skill content and role specialization into execution packets.
- Freeze raw and injected skill provenance into immutable run-start truth.
- Surface persisted execution-time skill truth through existing shell-owned readers.

### Locked Decisions

- External skills are Codex bundles rooted at `SKILL.md`.
- `skill_ref` and `skill_role` remain runtime-authoritative.
- Frozen skill truth belongs to immutable `Run` fields plus `RunStartSnapshot`.
- MVP fail-closes instead of truncating executable skill content.
- Visibility extends the existing shell-owned report / comparison / artifact spine.

### Acceptance Criteria

- Current external bundle shape resolves correctly.
- Resolved content reaches runtime execution packets.
- Raw and injected hashes freeze into run-start truth.
- Shell-owned visibility proof exists.
- Canonical same-tree `proposal-015` gate passes.

## Proposal Fidelity / Divergence

### Matches

- `SkillResolver`, `ExternalSkillLoader`, `SkillInjector`, `SkillRoleCustomizer`, and `RunStartSnapshot` remain present on the current tree.
- `scripts/test-gate.sh` still wires a dedicated `proposal-015` gate and a dedicated UI half.
- The current synced approved-host gate compiles `Proposal015Tests.swift` on the same tree before the shared regression stops the run.

### Divergences

- The fresh same-tree canonical gate does not pass. It fails in the non-UI half before Proposal 015 assertions execute.
- The required UI proof half is never reached, so no fresh same-tree UI proof exists for this audit.

### Fresh Basis Delta vs R6

- The old shell-script expansion blocker is closed in `scripts/test-gate.sh`.
- The approved host is reachable again over SSH.
- The live blocker moved upstream into the shared test target: `Proposal026Tests.swift` now breaks the `proposal-015` gate before the Proposal 015 slice can run.

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
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/ExternalSkillLoader.swift`
  - synced approved-host compile log from `./scripts/test-gate.sh proposal-015`
  - remote gate compiled `Chainworks ForgeTests/Proposal015Tests.swift` on the current synced tree before failing elsewhere
- Gap / Note: No fresh same-tree execution contradiction surfaced in the resolver path itself.

### REQ-002 Resolved skill content is injected into runtime execution packets
- Proposal Source: `§5.1-§5.5`, `§10 A1-A4`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - synced approved-host `proposal-015` build phase compiled the injection/runtime path on the current tree
- Gap / Note: The current blocker is not in packet injection; it is in a different test file inside the shared test target.

### REQ-003 Raw and injected skill hashes freeze into immutable run-start truth
- Proposal Source: `§6.1-§6.3`, `§10 A7-A8`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/Run.swift`
  - synced approved-host `proposal-015` build phase compiled the run-start truth path on the current tree
- Gap / Note: No fresh code-level contradiction surfaced against the provenance contract.

### REQ-004 Shell-owned readers expose persisted execution-time skill truth
- Proposal Source: `§6.3`, `§8.2`, `§10 A8-A10`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - synced approved-host `proposal-015` build phase compiled these shell-owned readers on the current tree
- Gap / Note: The implementation still routes visibility through the intended shell-owned surfaces.

### REQ-005 Approved-host UI proof surface executes successfully on the same tree
- Proposal Source: `§9 Phase 4`, `§10 A9-A11`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1219`
  - synced approved-host invocation: `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`
  - result bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-121208.xcresult`
- Gap / Note: The gate fails in the non-UI half with shared `Proposal026Tests.swift` compile errors, so the UI half never starts.

### REQ-006 Canonical proposal-owned proof lane passes on the same tree
- Proposal Source: `§9 Phase 4`, `§10 A9-A12`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `scripts/test-gate.sh:1113-1124`
  - remote invocation above
  - failure text:
    `No exact matches in call to initializer`
    `Cannot infer contextual base in reference to member 'empty'`
    `Cannot infer contextual base in reference to member 'operatorGrade'`
    `Cannot infer contextual base in reference to member 'legacyOperatorGrade'`
  - failing compile unit:
    `Chainworks ForgeTests/Proposal026Tests.swift`
- Gap / Note: The canonical same-tree proof lane exists and now launches correctly, but it is red before the Proposal 015 slice can finish.

## Architecture Review

**Summary:** `Acceptable`

No fresh architecture contradiction reopened the proposal. The implementation still centers skill resolution, injection, and provenance in the expected owners. The live blocker is proof-lane execution, not an architectural mismatch inside P015 itself.

## Product Review

**Summary:** `At Risk`

### PROD-001 Fresh same-tree proof is blocked before the proposal slice runs
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - synced approved-host `proposal-015` invocation above
  - result bundle above
- Why It Matters: The proposal promised reproducible execution-time proof, not just code presence. That proof is currently unavailable on the same tree.
- Recommended Action: Fix the shared `Proposal026Tests.swift` compile drift, then rerun the same synced approved-host `proposal-015` gate.

## UI Review

**Summary:** `At Risk`

### UI-001 The required UI proof half is blocked upstream
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`
- Evidence Type: `runtime`
- Evidence:
  - `scripts/test-gate.sh:1113-1124`
  - remote same-tree gate never advanced past `/var/folders/hh/.../proposal-015-non-ui-20260407-121208.xcresult`
- Why It Matters: Even though the UI proof hook still exists, this audit cannot claim a valid UI pass because the gate never reaches that half.
- Recommended Action: Clear the shared compile regression first, then rerun the canonical split gate through the UI phase.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Current local host still cannot execute the canonical `proposal-015` gate directly
- Severity: `Medium`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: `runtime`
- Evidence:
  - `scripts/test-gate.sh:354-385`
  - `scripts/test-gate.sh:1113-1116`
  - local host names:
    `hostname -> 0000659.localdomain`
    `LocalHostName -> 0000659`
    `ComputerName -> 0000659`
  - approved hosts:
    `SMacBook.local`, `SMacBook`
- Why It Matters: Local reruns of the canonical gate are still disallowed on this machine, so approved-host execution remains mandatory.
- Recommended Action: Keep using the approved-host lane for canonical proof, or expand the approved-host allowlist intentionally.

### READY-002 Approved-host same-tree gate is fresh-red before successful roll-up is possible
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - remote invocation above
  - non-UI result bundle above
  - failing compile unit: `Chainworks ForgeTests/Proposal026Tests.swift`
- Why It Matters: Under the current audit skill, a successful audit is impossible while the canonical same-tree proposal gate is red.
- Recommended Action: Repair `Proposal026Tests.swift`, then rerun the synced approved-host `proposal-015` gate before attempting another audit.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Fresh same-tree approved-host build phase completed before tests. |
| Core user flow runtime-validated | `Partial` | Canonical gate launched on the synced tree, but failed before Proposal 015 assertions executed. |
| Empty/loading/error states covered | `Not Checked` | No fresh UI execution reached those states in this pass. |
| Critical tests executed | `Fail` | Canonical same-tree `proposal-015` gate is red. |
| Approved-host UI proof passed on same tree | `Fail` | UI half never started. |
| Full regression suite passed on same tree/HEAD | `Not Run` | Not attempted after the proposal-owned gate already failed. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `hostname`
- `scutil --get LocalHostName`
- `scutil --get ComputerName`
- `ssh -o BatchMode=yes test@SMacBook.local 'hostname'`
- `rsync -az --delete --exclude='.git' --exclude='DerivedData' --exclude='.build' --exclude='*.xcresult' '/Users/user/Documents/Chainworks Forge/' 'test@SMacBook.local:/Users/test/chainworks-remote/'`
- `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`

## Recommended Next Actions

1. Fix the shared `Proposal026Tests.swift` compile drift that now blocks the canonical proposal gates.
2. Rerun the synced approved-host `proposal-015` gate once the shared test target is green again.
3. Only after the canonical gate passes, reassess whether a same-tree `full` regression run is needed for final successful roll-up.
