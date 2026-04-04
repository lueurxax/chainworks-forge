# Proposal 015 Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2de983d` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-04T07:35:21+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015` is no longer green on the current tree. The portability work is real: the repo now carries checked-in `examples/skills` bundles, the canonical example catalog points at repo-relative skill roots, path-localization helpers were widened, and the approved-host non-UI `proposal-015` slice passed `28/28` on the exact synced dirty tree. But the proposal-owned gate is still only partially implemented because the required UI proof lane now fails on the same approved-host tree: `testProposal015SkillVisibilityProofSurface` never establishes its XCTest connection and exits with an early bootstrap crash. The broader same-tree `full` gate is also red, so the audit cannot land on a successful readiness verdict.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Required `proposal-015` UI proof lane fails on the same synced tree | `High` |
| Architecture | `Acceptable` | Proposal-owned runtime/provenance design is in place; no new architecture blocker surfaced | `Medium` |
| Product | `At Risk` | Canonical operator proof for skill visibility is not operational on the approved host | `High` |
| UI | `At Risk` | `Chainworks ForgeUITests` crashes before bootstrapping the proof surface | `High` |
| UX | `Acceptable` | Shell-owned skill visibility path remains coherent in code and non-UI tests | `Medium` |
| Readiness | `Not Ready` | Proposal-owned gate is red and broader same-tree `full` regression is also red | `High` |

## Proposal Contract

### Scope

- Resolve `external_skill`, `inline_skill`, and `builtin_agent` bindings from catalog YAML.
- Inject resolved skill content and role specialization into execution packets.
- Freeze raw and injected skill provenance into immutable run-start truth.
- Persist execution-time skill truth and surface it through existing shell-owned readers.

### Locked Decisions

- `skill_ref` and `skill_role` are runtime-authoritative.
- External skills are Codex bundles rooted at `SKILL.md`.
- Frozen skill truth belongs to immutable `Run` fields plus `RunStartSnapshot`.
- MVP fail-closes instead of truncating executable skill content.
- Visibility extends current shell-owned report / comparison / artifact / readiness surfaces.

### Primary User Flows

1. Resolve builtin, inline, and external skills before execution starts.
2. Inject specialized skill content into Goose-backed execution packets.
3. Freeze raw and injected hashes at run start and persist injected truth on each execution row.
4. Inspect the same frozen skill truth from reports, comparisons, readiness, and artifact drilldown.

### UI Commitments

- Agent catalog shows skill type, role, content preview, and hashes.
- Pilot readiness shows skill preflight status.
- Report, comparison, and artifact readers expose resolved skill truth from persisted execution data.

### UX Commitments

- Invalid skill bindings block start through preflight.
- Role-specialized agents sharing a base skill get different execution prompts.
- Operators can inspect frozen skill truth without leaving the existing shell-owned surfaces.

### Acceptance Criteria

The proposal commits to external / inline / builtin resolution, role-differentiated prompt injection, blocking preflight on invalid skills, frozen raw and injected hashes, execution-time provenance, shell-owned operator visibility, UI smoke proof, and end-to-end coverage for all three skill types.

### Test / Evidence Requirements

- Integration proof that resolved skill content reaches execution packets.
- Preflight tests for missing external bundles and unknown builtin names.
- Provenance tests for raw vs injected hashes.
- UI proof that catalog, readiness, report, comparison, and artifact surfaces expose skill truth.
- Same-tree proposal-owned gate and, for any successful audit, same-tree `full` regression.

### Explicit Exclusions

- Skill authoring UI.
- Marketplace or hot-reload workflows.
- Skill versioning beyond content hashes.
- New provider integrations.

## Proposal Fidelity / Divergence

### Matches

- Checked-in repo-local skill bundles now exist at `examples/skills/proposal-review-triad/SKILL.md` and `examples/skills/proposal-implementation-audit/SKILL.md`.
- The canonical example catalog now points at `../skills/...` instead of workstation-specific absolute paths.
- Test and UI harnesses now localize legacy `/Users/user/.codex/skills/...`, `../skills/...`, and `../../examples/skills/...` roots into portable bundle copies.
- The approved-host non-UI `proposal-015` slice passed `28/28`.
- Frozen raw and injected skill provenance, execution-time persistence, and shell-owned visibility surfaces remain implemented.

### Divergences

- The proposal-owned UI proof lane is red on the current synced tree: `Chainworks ForgeUITests-Runner` exits before bootstrapping `testProposal015SkillVisibilityProofSurface`.
- Same-tree approved-host `full` regression is also red, so readiness is blocked even beyond the proposal-owned failure.

### Ambiguities / Evidence Gaps

- The approved-host `full` run was interrupted only after fresh red failures were already visible, so the red bundle is incomplete rather than a clean finished artifact. That does not weaken the blocker.

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
  - `examples/agents/agents.yaml:71-78`
  - `examples/skills/proposal-review-triad/SKILL.md`
  - `examples/skills/proposal-implementation-audit/SKILL.md`
  - `Chainworks ForgeTests/TestSupport.swift:215-235`
  - `Chainworks ForgeTests/Proposal015Tests.swift:661-690`
- Gap / Note: Current tree now ships repo-local external skill bundles and localized fixture rewrites for both legacy and repo-relative paths.

### REQ-002 Inline and builtin skills resolve to concrete runtime content

- Proposal Source: `§4.2`, `§4.3`, `A2`, `A3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/BuiltinSkillRegistry.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - approved-host non-UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-072949.xcresult`
- Gap / Note: The focused non-UI gate passed all `28` selected tests.

### REQ-003 Goose-backed execution injects resolved skill content before the agent prompt

- Proposal Source: `§5.1`, `§5.4`, `A1`, `A2`, `A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: Proposal-owned tests still assert preamble ordering and role-specialization divergence.

### REQ-004 Preflight blocks invalid skill bindings and readiness surfaces expose the result

- Proposal Source: `§7`, `§8.3`, `A5`, `A6`, `A11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1194-1273`
- Gap / Note: The current blocker is the UI harness bootstrap itself, not missing product logic for readiness rendering.

### REQ-005 Frozen run-start truth captures resolved raw and injected skill hashes

- Proposal Source: `§6.1`, `§6.3`, `A7`, `A8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: Raw and injected hashes remain frozen separately in run-start state.

### REQ-006 Execution rows persist injected skill provenance and shell-owned report surfaces consume it

- Proposal Source: `§6.3`, `§8.2`, `A8`, `A10`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks ForgeTests/Proposal015Tests.swift`
- Gap / Note: Persisted skill truth still feeds the current shell-owned readers.

### REQ-007 All three skill types have end-to-end coverage from YAML through execution and provenance

- Proposal Source: `§9` phases 1-3, `A12`
- Status: `Implemented`
- Evidence Type: `tests-run`
- Evidence:
  - `Chainworks ForgeTests/Proposal015Tests.swift`
  - approved-host non-UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-072949.xcresult`
- Gap / Note: The proposal-owned non-UI slice passed `28/28`.

### REQ-008 Canonical proposal-owned gate passes on the same synced tree

- Proposal Source: `§9` phase 4, `A9`, `A10`, `A11`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `scripts/test-gate.sh:878-889`
  - approved-host non-UI summary: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-072949.xcresult`
  - approved-host UI summary: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-073025.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1194-1273`
- Gap / Note: The gate exists and its non-UI half is green, but the required UI proof half failed `0/1` with `Early unexpected exit, operation never finished bootstrapping - no restart will be attempted`.

## Architecture Review

**Summary:** `Acceptable`

No new proposal-owned architecture finding remains on current evidence. The current tree preserves the proposal’s intended owner model around frozen run-start truth, injected skill provenance, and shell-owned inspection surfaces.

## Product Review

**Summary:** `At Risk`

### PROD-001 Canonical skill-visibility proof is not operational on the approved host

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host UI summary: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-073025.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1194-1273`
- Why It Matters: The proposal explicitly requires an app-level proof lane for skill visibility. When the proof runner crashes before bootstrapping, the product claim is not operational even if the underlying data model is present.
- Recommended Action: Debug the approved-host `Chainworks ForgeUITests` bootstrap failure first, then rerun `./scripts/test-gate.sh proposal-015` on the same tree.

## UI Review

**Summary:** `At Risk`

### UI-001 App proof surface crashes before XCTest can attach

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host UI summary: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-073025.xcresult`
  - failure text: `Chainworks ForgeUITests-Runner (26189) encountered an error`
  - failure text: `Early unexpected exit, operation never finished bootstrapping - no restart will be attempted`
- Why It Matters: The current tree no longer has a passing UI proof for the proposal’s shell-owned skill visibility contract. That is a direct proposal-owned gap, not just a readiness nicety.
- Recommended Action: Stabilize the UI test runner on the approved host and re-establish the `P015_Skill_Truth_Proof` lane before calling the proposal implemented.

## UX Review

**Summary:** `Acceptable`

No new proposal-owned UX finding remains on current evidence. The shell-owned visibility route is still coherent in code and non-UI coverage; the current blocker is proof execution rather than an observed interaction-design regression.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 The proposal-owned gate is red on the same synced tree

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `tests-run`
- Evidence:
  - approved-host command: `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-dual-20260404' && ./scripts/test-gate.sh proposal-015"`
  - non-UI summary: `28` passed, `0` failed
  - UI summary: `0` passed, `1` failed
- Why It Matters: The proposal’s own canonical gate is a release criterion. It is not green on the audited tree.
- Recommended Action: Fix the UI bootstrap crash and rerun the split gate on the same synced tree.

### READY-002 Same-tree approved-host `full` regression is also red

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: successful audit roll-up gate
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host sync root: `/Users/test/chainworks-audit-2de983d-dual-20260404`
  - approved-host `full` command: `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-dual-20260404' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='***' ./scripts/test-gate.sh full"`
  - live red failures in `ProviderPlatformTests.swift:1572`, `FullMVPDeliveryTests.swift:1083-1084`, and `FullMVPDeliveryTests.swift:1209-1216`
  - incomplete red bundle path: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260404-073234.xcresult`
- Why It Matters: Even if the proposal-owned UI proof were green, the current audit skill forbids a successful verdict without passing same-tree `full` regression.
- Recommended Action: Clear the fresh repo-level `full` failures, then rerun this audit on the same tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Approved-host `proposal-015` build phase succeeded before split execution |
| Core user flow runtime-validated | `Partial` | Non-UI slice passed; UI proof lane failed before bootstrapping |
| Empty/loading/error states covered | `Not Checked` | Not a primary proposal-owned focus in this pass |
| Accessibility risk acceptable | `Not Checked` | No dedicated accessibility audit was run |
| Localization risk acceptable | `Not Checked` | Not reviewed in this pass |
| Critical tests executed | `Pass` | Both proposal-owned split halves were executed on the approved host |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | Approved-host same-tree `full` went red before completion |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Outside the bounded proposal scope |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `date '+%Y-%m-%dT%H:%M:%S%z'`
- `md5 -q 'docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `tar czf - -C '/Users/user/Documents/Chainworks Forge' --exclude .git --exclude .codex . | ssh test@SMacBook.local "rm -rf '/Users/test/chainworks-audit-2de983d-dual-20260404' && mkdir -p '/Users/test/chainworks-audit-2de983d-dual-20260404' && tar xzf - -C '/Users/test/chainworks-audit-2de983d-dual-20260404'"`
- same-tree parity spot-checks:
  - local / remote `md5 examples/agents/agents.yaml` = `308643ff946473f9c390fcc7c7b35711`
  - local / remote `md5 'Chainworks ForgeTests/TestSupport.swift'` = `fa991bcf537ecf180d2b6520fe91fb8a`
  - local / remote proposal MD5 = `162789b1c6a3b41439c7e4d6d72b436c`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-dual-20260404' && ./scripts/test-gate.sh proposal-015"`
- `ssh test@SMacBook.local "xcrun xcresulttool get test-results summary --path '/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260404-072949.xcresult'"`
- `ssh test@SMacBook.local "xcrun xcresulttool get test-results summary --path '/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260404-073025.xcresult'"`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-dual-20260404' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='***' ./scripts/test-gate.sh full"`

## Recommended Next Actions

1. Fix the approved-host `Chainworks ForgeUITests` bootstrap crash in `testProposal015SkillVisibilityProofSurface`.
2. Rerun `./scripts/test-gate.sh proposal-015` on the same synced tree until both non-UI and UI halves are green.
3. After that, clear the fresh same-tree `full` failures and rerun this audit to determine whether `P015` can move to a successful roll-up.
