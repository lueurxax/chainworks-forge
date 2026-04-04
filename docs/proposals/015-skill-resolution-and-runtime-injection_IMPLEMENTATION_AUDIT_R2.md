# Proposal 015 Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2de983d` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-03T23:25:24+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015`'s proposal-owned slice is implemented on the current tree. The same-tree focused gate passed on a synced approved-host checkout: `./scripts/test-gate.sh proposal-015` returned `RC=0`, the non-UI half passed in `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260403-233513.xcresult`, and the UI proof lane passed `testProposal015SkillVisibilityProofSurface` in `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260403-233548.xcresult` with the watchdog accepting the post-success hang. The runtime path now resolves skills, injects specialized content into execution prompts, freezes raw and injected hashes, persists execution truth, and surfaces that truth in the existing shell-owned views.

The audit still cannot land on a successful roll-up. Under the current audit skill, any `Implemented` / `Ready` verdict requires a passing same-tree full regression run. I attempted that on the same synced approved-host checkout via `./scripts/test-gate.sh full`, but the run went red before completion. The full-suite log at `/tmp/p015-full-20260403-232756.log` already recorded multiple failing tests, including absolute-path skill-resolution failures against `/Users/user/.codex/skills/...` and a UI failure in `testArtifactInspectorOpensProposalAndReceiptArtifacts`. That means `P015` itself is green at the focused slice level, but the repo is not green enough on the same tree for a successful audit verdict.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Proposal-owned slice is green, but success fails closed without passing same-tree full regression | `High` |
| Architecture | `At Risk` | External skill runtime truth still depends on host-specific absolute filesystem roots outside localized fixtures | `High` |
| Product | `At Risk` | A synced checkout on the approved host cannot run broader canonical flows without separately staging local skill bundles | `High` |
| UI | `Acceptable` | Proposal-owned UI proof surface passed on the approved host | `Medium` |
| UX | `Acceptable` | Proposal-owned operator visibility path is coherent and runtime-validated in the focused gate | `Medium` |
| Readiness | `Not Ready` | Same-tree full regression on the approved host is red | `High` |

## Proposal Contract

### Scope

- Skill resolution for `external_skill`, `inline_skill`, and `builtin_agent`
- Runtime injection of resolved skill content into agent execution context
- `skill_role` specialization, including mode-based `proposal_review_triad` mapping
- Frozen run-start skill provenance and drift-detectable hashing
- Skill preflight validation and operator visibility in existing shell-owned surfaces

### Locked Decisions

- `skill_ref` and `skill_role` are runtime-authoritative, not metadata-only
- External skills are Codex bundles rooted at `SKILL.md`
- `Run` frozen fields plus `RunStartSnapshot` are the immutable owner for resolved raw and injected skill truth
- MVP fail-closes rather than truncating executable skill content
- Operator visibility extends `RunReportView`, `RunComparisonView`, and `ArtifactInspectorView`; no parallel inspection lane

### Primary User Flows

1. Resolve built-in, inline, and external skills from catalog YAML before execution starts.
2. Inject resolved skill content and role specialization into the execution prompt delivered to Goose-backed sessions.
3. Freeze raw and injected skill truth into run-start state and persist the injected hash onto each execution row.
4. Inspect the same skill truth through preflight, agent catalog, reports, comparisons, and artifact drilldown surfaces.

### UI Commitments

- Agent catalog shows skill type, role, content preview, and hashes
- Pilot readiness shows skill preflight status
- Run reports, comparisons, and artifact inspector expose resolved skill truth from persisted execution/run data

### UX Commitments

- Missing external skill paths and unknown builtin names block start via preflight
- Role-specialized agents sharing a base skill receive different execution prompts
- Operators can inspect the frozen skill contract without leaving the existing shell-owned surfaces

### Acceptance Criteria

The proposal commits to external / inline / builtin resolution, role-differentiated prompt injection, blocking preflight on invalid skills, frozen raw and injected hashes, execution-time provenance, shell-owned operator visibility, UI smoke proof, and end-to-end coverage for all three skill types.

### Test / Evidence Requirements

- Integration proof that resolved skill content reaches execution packets
- Preflight tests for missing external bundles and unknown builtin names
- Snapshot / provenance tests for raw vs injected hashes
- UI proof that catalog, readiness, report, comparison, and artifact surfaces expose skill truth
- Same-tree proposal-owned gate and, for any successful audit, same-tree full regression

### Explicit Exclusions

- Skill authoring UI
- Marketplace / package manager / hot reload
- Skill versioning beyond content hashes
- New provider integrations
- Transport-level permission-profile enforcement and other Tier 3 runtime settings propagation

## Proposal Fidelity / Divergence

### Matches

- `SkillResolver`, `ExternalSkillLoader`, `BuiltinSkillRegistry`, `SkillRoleCustomizer`, and `SkillInjector` are all live.
- `RunPlanCompiler` resolves each referenced agent's `skill_ref` and stores `ResolvedSkill` on `ResolvedAgent`.
- `GooseSessionBridge.buildExecutionPacket()` injects resolved skill content before the agent prompt.
- `Run`, `RunStartSnapshot`, and execution rows persist resolved raw and injected skill truth.
- Existing shell-owned report, comparison, artifact, and readiness surfaces expose that persisted truth.
- Proposal-owned focused proof is green on the approved host on the same synced tree.

### Divergences

- The broader repo still depends on host-specific absolute external skill paths in the canonical example catalog.
- The same-tree approved-host full regression run is red, so the audit must fail closed even though the narrow proposal slice is green.

### Ambiguities / Evidence Gaps

- I aborted the red full regression after the failures were already evident in `/tmp/p015-full-20260403-232756.log`, so there is no completed green `full` footer to cite. That is enough to block a successful audit under the current rule.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

Because the current audit skill requires passing same-tree full regression before any successful roll-up, `Overall Conformance` still fail-closes to `Partial` despite every proposal-owned requirement below being implemented.

## Requirement Audit

### REQ-001 External skill bundles resolve from Codex `SKILL.md` roots with current role mapping

- Proposal Source: `§4.1`, `§5.3`, `A1`, `A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/ExternalSkillLoader.swift:3-35`
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift:72-128`
  - `Chainworks Forge/Engine/Skills/SkillRoleCustomizer.swift:3-76`
  - `Chainworks ForgeTests/Proposal015Tests.swift:26-44`
  - `/tmp/p015-gate-20260403-233444.log`
- Gap / Note: The focused approved-host gate log shows the `Proposal 015` suite passing on the synced tree.

### REQ-002 Inline and builtin skills resolve to concrete runtime content

- Proposal Source: `§4.2`, `§4.3`, `A2`, `A3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift:84-100`
  - `Chainworks Forge/Engine/Skills/BuiltinSkillRegistry.swift:3-19`
  - `Chainworks ForgeTests/Proposal015Tests.swift:335-378`
  - `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260403-233513.xcresult`
- Gap / Note: Inline and builtin end-to-end paths are covered by the same focused suite.

### REQ-003 Goose-backed execution injects resolved skill content before the agent prompt

- Proposal Source: `§5.1`, `§5.4`, `A1`, `A2`, `A4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift:3-15`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:337-351`
  - `Chainworks ForgeTests/Proposal015Tests.swift:47-99`
  - `Chainworks ForgeTests/Proposal015Tests.swift:432-518`
- Gap / Note: The proposal-owned tests explicitly assert skill preamble ordering and role-specialization prompt divergence.

### REQ-004 Preflight blocks invalid skill bindings and readiness surfaces expose the result

- Proposal Source: `§7`, `§8.3`, `A5`, `A6`, `A11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:240-297`
  - `Chainworks Forge/Views/PilotReadinessView.swift:118-140`
  - `Chainworks ForgeTests/Proposal015Tests.swift:101-151`
  - `Chainworks ForgeTests/Proposal015Tests.swift:380-431`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1228-1234`
  - `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260403-233548.xcresult`
- Gap / Note: The approved-host UI proof explicitly validates the pilot-readiness skill section.

### REQ-005 Frozen run-start truth captures resolved raw and injected skill hashes

- Proposal Source: `§6.1`, `§6.3`, `A7`, `A8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/Run.swift:50-52`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:5-44`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:74-115`
  - `Chainworks ForgeTests/Proposal015Tests.swift:153-205`
- Gap / Note: The round-trip test proves raw resolved hashes and injected hashes are both frozen in run-start state.

### REQ-006 Execution rows persist injected skill provenance and shell-owned report surfaces consume it

- Proposal Source: `§6.3`, `§8.2`, `A8`, `A10`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2752-2766`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:155-179`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:560-575`
  - `Chainworks Forge/Engine/RunComparisonService.swift:157-170`
  - `Chainworks Forge/Views/RunComparisonView.swift:394-428`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:220-255`
  - `Chainworks ForgeTests/Proposal015Tests.swift:207-333`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1237-1265`
- Gap / Note: The proposal-owned UI proof validates report/comparison/artifact visibility, and unit tests validate the persisted hashes and content.

### REQ-007 All three skill types have end-to-end coverage from YAML through execution and provenance

- Proposal Source: `§9` phases 1-3, `A12`
- Status: `Implemented`
- Evidence Type: `tests-run`
- Evidence:
  - `Chainworks ForgeTests/Proposal015Tests.swift:335-378`
  - `Chainworks ForgeTests/Proposal015Tests.swift:432-518`
  - `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260403-233513.xcresult`
- Gap / Note: The focused proposal suite covers inline, builtin, and external skill flows plus role-specialization divergence.

### REQ-008 Canonical proposal-owned gate passes on the same synced tree

- Proposal Source: `§9` phase 4, `A9`, `A10`, `A11`
- Status: `Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `scripts/test-gate.sh:71-74`
  - `scripts/test-gate.sh:748`
  - `scripts/test-gate.sh:871-882`
  - `/tmp/p015-gate-20260403-233444.log`
  - `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260403-233513.xcresult`
  - `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260403-233548.xcresult`
- Gap / Note: The gate returned `RC=0`; the UI half passed `testProposal015SkillVisibilityProofSurface`, and the built-in watchdog accepted the post-success xcodebuild hang.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 External skill truth still depends on host-specific absolute filesystem roots outside localized proof fixtures

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-004`, `REQ-008`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `examples/agents/agents.yaml:72-78`
  - `Chainworks ForgeTests/Proposal015Tests.swift:642-672`
  - `Chainworks Forge/Support/PreviewSupport.swift:408-415`
  - `/tmp/p015-full-20260403-232756.log`
- Why It Matters: The proposal-owned slice works because tests localize external-skill paths into temporary bundles. Broader repo paths still point at `/Users/user/.codex/skills/...`, which does not exist on the approved host synced checkout. That keeps the wider system dependent on host-specific filesystem shape and causes broader preview / end-to-end / resume flows to fail even though the narrow P015 mechanism is correct.
- Recommended Action: Introduce a canonical skill-root indirection for the example catalog, or bootstrap the required external skill bundles into the approved-host checkout before running full sign-off gates. The important change is to stop depending on `/Users/user/...` absolute paths outside localized fixtures.

## Product Review

**Summary:** `At Risk`

### PROD-001 The canonical example catalog is not portable enough for approved-host sign-off

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-008`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `examples/agents/agents.yaml:72-78`
  - `/tmp/p015-full-20260403-232756.log`
  - `Chainworks ForgeTests/Proposal015Tests.swift:642-672`
- Why It Matters: An engineer can sync the current tree to the approved host and pass the narrow `proposal-015` gate, but the broader product flows still fail because the canonical example catalog expects local Codex skill bundles that are not present there. That weakens the product claim that the catalog-driven skill system is operational in the repo’s real sign-off environment.
- Recommended Action: Make the example catalog resolve skills through portable environment-backed paths or a checked-in test-skill bootstrap path, then rerun the approved-host `full` gate on the same synced tree.

## UI Review

**Summary:** `Acceptable`

No live proposal-owned UI finding remains. The approved-host proof lane validated the catalog, readiness, report, comparison, and artifact visibility path for skill truth.

## UX Review

**Summary:** `Acceptable`

No live proposal-owned UX finding remains. The focused approved-host gate confirms the existing shell-owned visibility route works end to end for the P015 slice.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Same-tree full regression on the approved host is red

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: audit readiness rule
- Evidence Type: `tests-run`
- Evidence:
  - `tar czf - -C "/Users/user/Documents/Chainworks Forge" --exclude .git --exclude .codex . | ssh test@SMacBook.local "rm -rf /Users/test/chainworks-audit-2de983d-p015 && mkdir -p /Users/test/chainworks-audit-2de983d-p015 && tar xzf - -C /Users/test/chainworks-audit-2de983d-p015"`
  - `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-p015' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD=*** ./scripts/test-gate.sh full"`
  - `/tmp/p015-full-20260403-232756.log`
- Why It Matters: The current audit skill explicitly forbids successful verdicts without passing same-tree full regression. The approved-host full suite recorded multiple failures before completion, including missing external skill bundles on the synced host and a UI failure in `testArtifactInspectorOpensProposalAndReceiptArtifacts`. That is enough to block `Implemented`, `Ready`, and `Ready with Risks`.
- Recommended Action: Fix the approved-host full-suite failures first, then rerun `./scripts/test-gate.sh full` on the same synced tree/HEAD.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | The approved-host `proposal-015` gate built and passed both non-UI and UI halves on the synced tree. |
| Core user flow runtime-validated | `Pass` | `testProposal015SkillVisibilityProofSurface` passed on the approved host and attached `P015_Skill_Truth_Proof`. |
| Empty/loading/error states covered | `Not Checked` | Not a proposal-owned focus for this audit. |
| Accessibility risk acceptable | `Not Checked` | No dedicated accessibility run beyond the proposal-owned UI proof. |
| Localization risk acceptable | `Not Checked` | No localization-specific verification was run. |
| Critical tests executed | `Pass` | `./scripts/test-gate.sh proposal-015` passed on the same synced checkout (`RC=0`). |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | The approved-host `full` attempt on the same synced checkout went red before completion; see `/tmp/p015-full-20260403-232756.log`. |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Outside the bounded proposal scope. |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `date '+%Y-%m-%dT%H:%M:%S%z' && md5 -q 'docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `rg -n "superseded|deprecated|replaced by|obsolete" -S 'docs/proposals/015-skill-resolution-and-runtime-injection.md' 'docs/proposals' 'docs/reviews'`
- `rg -n "ResolvedSkill|SkillResolver|ExternalSkillLoader|BuiltinSkillRegistry|SkillInjector|SkillRoleCustomizer|resolvedSkill|skillSnapshotHash|resolvedSkillsJSON|skillContentHashes|skillInjectedContentHashes" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n "proposal-015|PROPOSAL_015|Proposal015Tests" 'scripts/test-gate.sh' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `tar czf - -C "/Users/user/Documents/Chainworks Forge" --exclude .git --exclude .codex . | ssh test@SMacBook.local "rm -rf /Users/test/chainworks-audit-2de983d-p015 && mkdir -p /Users/test/chainworks-audit-2de983d-p015 && tar xzf - -C /Users/test/chainworks-audit-2de983d-p015"`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-p015' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD=*** ./scripts/test-gate.sh full > /tmp/p015-full-20260403-232756.log 2>&1"`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-p015' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD=*** ./scripts/test-gate.sh proposal-015 > /tmp/p015-gate-20260403-233444.log 2>&1"`

## Recommended Next Actions

1. Remove the hard dependency on `/Users/user/.codex/skills/...` absolute paths in the canonical example catalog, or bootstrap those bundles into the approved-host checkout before full sign-off.
2. Re-run the approved-host `full` gate on the same synced tree after the broader failures are fixed.
3. Once `full` is green, re-run this audit to upgrade the roll-up from fail-closed `Partial` / `Not Ready`.
