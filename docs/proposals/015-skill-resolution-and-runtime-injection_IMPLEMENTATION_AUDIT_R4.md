# Proposal 015 Implementation Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `73e4169` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-04T12:23:56+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015` remains `Partial` on the current tree. The core proposal-owned implementation is still real: the local same-tree non-UI slice passed `29/29` at `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-nonui-YljHVO.xcresult`, and the exact synced approved-host copy of the same dirty tree also passed the canonical non-UI half `29/29` at `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-121943.xcresult`. The blocker is still the required app-level proof lane: the approved-host UI half failed `0/1` at `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-122021.xcresult` because `Chainworks ForgeUITests-Runner` exited before establishing its XCTest connection. Under the updated audit rule, the same-tree approved-host `full` gate also stayed red, so `P015` cannot roll up to a successful verdict.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Required `proposal-015` UI proof lane is red on the same synced tree | `High` |
| Architecture | `Acceptable` | Frozen skill truth, injected-content hashing, and shell-owned visibility remain in place | `Medium` |
| Product | `At Risk` | Canonical operator proof for execution-time skill visibility is not operational | `High` |
| UI | `At Risk` | `Chainworks ForgeUITests` still crashes before bootstrapping the proof surface | `High` |
| UX | `Acceptable` | Shell-owned inspection flow remains coherent in code and non-UI coverage | `Medium` |
| Readiness | `Not Ready` | Proposal-owned UI proof is red and same-tree `full` is red | `High` |

## Proposal Contract

### Scope

- Resolve `external_skill`, `inline_skill`, and `builtin_agent` bindings from catalog YAML.
- Inject resolved skill content and role specialization into execution packets.
- Freeze raw and injected skill provenance into immutable run-start truth.
- Persist execution-time skill truth and expose it through existing shell-owned report, comparison, artifact, and readiness surfaces.

### Locked Decisions

- `skill_ref` and `skill_role` are runtime-authoritative.
- External skills are Codex bundles rooted at `SKILL.md`.
- Frozen skill truth belongs to immutable `Run` fields plus `RunStartSnapshot`.
- MVP fail-closes instead of truncating executable skill content.
- Visibility extends current shell-owned operator surfaces instead of opening a parallel lane.

### Primary User Flows

1. Resolve builtin, inline, and external skills before execution starts.
2. Inject specialized skill content into Goose-backed execution packets.
3. Freeze raw and injected hashes at run start and persist injected truth on execution rows.
4. Inspect the same frozen skill truth from reports, comparisons, readiness, and artifact drilldown.

### UI / UX Commitments

- Agent catalog and readiness surfaces show resolved skill type, role, and preflight truth.
- Report, comparison, and artifact readers expose persisted skill truth from execution records.
- Invalid skill bindings block launch through preflight.
- Role-specialized agents sharing a base skill get different execution prompts.

### Test / Evidence Requirements

- Integration proof that resolved skill content reaches execution packets.
- Preflight tests for missing external bundles and unknown builtin names.
- Provenance tests for raw vs injected hashes.
- App-level UI proof for shell-owned skill visibility.
- Same-tree proposal-owned gate and, for any successful audit, same-tree `full` regression.

## Proposal Fidelity / Divergence

### Matches

- Repo-local external skill bundles still exist under `examples/skills/.../SKILL.md`.
- The current tree still freezes both raw and injected skill hashes into run-start truth.
- Execution rows and shell-owned readers still persist and consume injected skill truth.
- The exact synced approved-host non-UI `proposal-015` slice passed `29/29`.
- The local same-tree corroborating non-UI slice also passed `29/29`.

### Divergences

- The required approved-host UI proof lane is still red on the exact same tree.
- The same-tree approved-host `full` run also went red before completion, so readiness cannot succeed even apart from the proposal-owned UI failure.

### Ambiguities / Evidence Gaps

- The approved-host `full` run was interrupted after fresh red failures were already visible, so the `full-20260404-122221.xcresult` bundle is incomplete and not a clean result-bundle artifact.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 External skill bundles resolve from Codex `SKILL.md` roots with current role mapping

- Proposal Source: `§4.1`, `§5.3`, `A1`, `A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `examples/skills/proposal-review-triad/SKILL.md`
  - `examples/skills/proposal-implementation-audit/SKILL.md`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - local same-tree bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-nonui-YljHVO.xcresult`
- Gap / Note: Current tree still resolves the checked-in Codex bundle shape and current mode-based proposal-review specialization.

### REQ-002 Inline and builtin skills resolve to concrete runtime content

- Proposal Source: `§4.2`, `§4.3`, `A2`, `A3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/BuiltinSkillRegistry.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - approved-host non-UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-121943.xcresult`
- Gap / Note: Focused same-tree proof covers builtin, inline, and external resolution paths together.

### REQ-003 Goose-backed execution injects resolved skill content before the agent prompt

- Proposal Source: `§5.1`-`§5.4`, `A1`, `A2`, `A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
  - local same-tree bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-nonui-YljHVO.xcresult`
- Gap / Note: Current non-UI coverage still proves preamble ordering, role specialization, and exact injected payload truth.

### REQ-004 Preflight blocks invalid skill bindings and readiness surfaces expose the result

- Proposal Source: `§4.4`, `§7`, `§8.3`, `A5`, `A6`, `A11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Gap / Note: The current problem is UI harness bootstrapping, not missing product logic for readiness rendering.

### REQ-005 Frozen run-start truth captures resolved raw and injected skill hashes

- Proposal Source: `§3` Layer B, `§6.1`, `§6.3`, `A7`, `A8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - local same-tree bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-nonui-YljHVO.xcresult`
- Gap / Note: Current tree still freezes both `skillContentHashesJSON` and `skillInjectedContentHashesJSON`.

### REQ-006 Execution rows persist injected skill provenance and shell-owned report surfaces consume it

- Proposal Source: `§3` Layer D, `§6.3`, `§8.2`, `A8`, `A10`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: Current tree still writes injected skill truth into execution/report surfaces instead of reconstructing it heuristically.

### REQ-007 All three skill types have end-to-end coverage from YAML through execution and provenance

- Proposal Source: `§2`, `§9` phases 1-3, `A12`
- Status: `Implemented`
- Evidence Type: `tests-run`
- Evidence:
  - local same-tree bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p015-nonui-YljHVO.xcresult`
  - approved-host non-UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-121943.xcresult`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: Both current-tree non-UI proof lanes passed `29/29`.

### REQ-008 Canonical proposal-owned gate passes on the same synced tree

- Proposal Source: `§9` phase 4, `A9`, `A10`, `A11`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `scripts/test-gate.sh`
  - approved-host non-UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-121943.xcresult`
  - approved-host UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-122021.xcresult`
  - failure text: `Early unexpected exit, operation never finished bootstrapping - no restart will be attempted. (Underlying Error: Test crashed with signal kill before establishing connection.)`
- Gap / Note: The gate exists and the non-UI half is green, but the required UI proof half failed `0/1` on the exact synced dirty tree.

## Architecture Review

**Summary:** `Acceptable`

No new proposal-owned architecture gap surfaced on the current tree. The owner model around resolved skill truth, injected-content hashing, and shell-owned visibility remains consistent with the proposal.

## Product Review

**Summary:** `At Risk`

### PROD-001 Canonical operator proof for skill visibility is still non-operational

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-122021.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Why It Matters: The proposal explicitly promised an app-level proof lane that shows execution-time skill truth through current shell-owned surfaces. That product claim is still not operational on the approved host.
- Recommended Action: Fix the `Chainworks ForgeUITests` bootstrap failure first, then rerun `./scripts/test-gate.sh proposal-015` on the same synced tree.

## UI Review

**Summary:** `At Risk`

### UI-001 `Chainworks ForgeUITests-Runner` still exits before XCTest can attach

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-122021.xcresult`
  - failure text: `Chainworks ForgeUITests-Runner (36847) encountered an error`
  - failure text: `Test crashed with signal kill before establishing connection`
- Why It Matters: This is the proposal’s own proof surface, so a harness bootstrap crash is a direct proposal-owned blocker rather than a generic UI-testing inconvenience.
- Recommended Action: Stabilize the approved-host runner bootstrap and re-establish the `P015` proof lane before calling the proposal implemented.

## UX Review

**Summary:** `Acceptable`

No new proposal-owned UX gap surfaced on the current evidence. Operators still have a coherent shell-owned path for skill truth in code and non-UI coverage; the current blocker is proof execution, not an observed interaction regression.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Same-tree readiness is blocked by both the proposal-owned UI gate and the broader full regression gate

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host `proposal-015` UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-122021.xcresult`
  - approved-host terminal output from `./scripts/test-gate.sh full` on `/Users/test/chainworks-audit-73e4169-dual-20260404`
  - fresh failures observed in `ProviderPlatformTests`, `ExecutionServiceTests`, and `MVPGoldenRunTests` before the post-failure run was stopped
- Why It Matters: Under the current audit skill, any successful verdict requires a passing same-tree `full` regression in addition to focused proposal proof. Current tree has neither.
- Recommended Action: Fix the UI bootstrap failure first, then rerun both `proposal-015` and `full` on the exact same synced tree.
