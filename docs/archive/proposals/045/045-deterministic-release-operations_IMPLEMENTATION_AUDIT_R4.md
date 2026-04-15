# Proposal 045: Deterministic Release Operations (Git + Sandbox Publish) Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | docs/proposals/045-deterministic-release-operations.md |
| Repository Root | . |
| Git SHA | ddc5c0d |
| Working Tree | dirty (0 index, 4717 worktree, 102562 untracked, 0 ignored) |
| Audited At | 2026-04-15T10:30:59+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready with Risks |
| Audit Confidence | High |

## Executive Verdict

P045 remains implemented on the audited tree, and the basis is materially stronger than in `R3`. The proposal-owned `proposal-045` gate is still green on the same `HEAD`, and the old executor failure-path evidence gap is now closed: the release suite directly proves structured `delivery_receipt` persistence on both git failure and publish failure. The verdict stays `Ready with Risks`, but the remaining risk is narrower and now limited to still-unexecuted edge cases such as missing-config fail-close and preserve-without-overwrite behavior, not to the core release path itself.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | A few edge-case receipt branches remain code-proved rather than directly executed | High |
| Architecture | Acceptable | Release truth still spans executor routing, artifact-path resolution, and terminal receipt backfill | High |
| Product | Acceptable | Core deterministic release behavior is now covered on happy and release-attempt failure paths | High |
| UI | Acceptable | No proposal-specific UI surface is in scope | Medium |
| UX | Acceptable | No new operator interaction surface is introduced by this proposal | Medium |
| Readiness | Ready with Risks | Missing-config and preserve-without-overwrite receipt branches are not yet isolated by focused runtime proof | High |

## Proposal Contract

### Scope

- Port Swift `GitReleaseService`, `ConnectPublishService`, `ReleaseOpsCoordinator`, and `DeliveryReceiptBuilder` to the Rust daemon.
- Add `delivery_configuration_json` through `StartRunCmd`, command handler, and northbound surfaces so frozen delivery config reaches release execution.
  Source: `docs/proposals/045-deterministic-release-operations.md:8-10`

### Locked Decisions

- Release agents execute through native Rust services, not ACP.
- `delivery_configuration_json` is frozen at run start and deserialized fail-closed at release time.
- Git and publish execute as separate per-agent steps.
- `delivery_receipt` is written on happy paths and release-attempt failure paths, with terminal backfill only when prior release-agent lineage exists.
- Release mode stays sandbox/staging safe mode only; no real App Store Connect communication.
  Source: `docs/proposals/045-deterministic-release-operations.md:12-25`, `docs/proposals/045-deterministic-release-operations.md:226-260`

### Primary User Flows

1. Start a repo-backed run with frozen `delivery_configuration_json` and preserve that config on the run.
2. Execute `commit_and_push_to_github` natively, produce canonical git release artifacts, and write a real commit to the release branch.
3. Execute `build_archive_and_push_connect` natively using prior git artifacts, produce canonical publish artifacts, and persist `delivery_receipt`.
4. Preserve structured release truth on git failure, publish failure, and lineage-gated terminal backfill paths.
5. Expose frozen delivery config and release evidence through current northbound read surfaces.

### UI Commitments

- No direct UI commitments. The proposal is daemon/release-path implementation only.

### UX Commitments

- Release execution is deterministic and non-LLM.
- Missing config/worktree fails closed.
- Failure paths preserve structured release truth instead of synthesizing metadata-only success-like receipts.
- Terminal backfill is lineage-gated, not metadata-only.
  Source: `docs/proposals/045-deterministic-release-operations.md:226-260`, `docs/proposals/045-deterministic-release-operations.md:307-321`

### Acceptance Criteria

1. `delivery_configuration_json` accepted at MCP `runs.start` and GraphQL `startRun`, persisted on `Run`, deserialized at release time.
2. `commit_and_push_to_github` produces `release_manifest.json` and `git_push_receipt.json` without ACP.
3. `build_archive_and_push_connect` consumes git artifacts and produces `release_bundle_manifest.json` and `connect_upload_receipt.json`.
4. `delivery_receipt.json` is persisted at canonical path on happy paths and release-attempt failure paths, with correct `ReleaseResultSummary`.
5. `git log` on target branch shows the commit from the daemon.
6. All artifacts are written to canonical paths so `exists('git_push_receipt')` evaluates true.
7. Git failure records structured `ReleaseResultSummary { succeeded: false, failure_stage: "commit_and_push" }`.
8. Publish failure preserves git artifacts, records structured failure truth, and blocks the run.
9. Missing `delivery_configuration_json` fails closed and produces no executor-side receipt on that pre-release path.
10. Preserve-vs-backfill: an existing `delivery_receipt` is not overwritten.
11. Backfill eligibility requires `delivery_config`, `worktree_root`, and prior release-agent lineage.
12. No LLM is involved in the release path.
13. `connect_upload_receipt.release_mode` is `sandbox` or `staging`, never `production`.
  Source: `docs/proposals/045-deterministic-release-operations.md:292-321`

### Test / Evidence Requirements

- Add a repo-owned `proposal-045` gate in `scripts/test-gate.sh`.
- Add a matching `docs/reference/test-gates.md` entry.
- Cover input path, git release, sandbox publish, partial failure semantics, delivery receipt behavior, native executor routing, and protected-branch rejection.
  Source: `docs/proposals/045-deterministic-release-operations.md:323-363`

### Explicit Exclusions

- Real App Store Connect upload.
- Production release mode.
- Broader post-approval orchestration beyond P044.
- UI/CLI repo-profile selection.
  Source: `docs/proposals/045-deterministic-release-operations.md:365-370`

## Proposal Fidelity / Divergence

### Matches

- `delivery_configuration_json` is wired through domain commands, command handling, MCP start, and GraphQL start.
- Rust-native release modules exist for git, publish, coordinator, and receipt building.
- The executor routes release agents natively and bypasses ACP.
- Git, publish, and `delivery_receipt` artifacts resolve through canonical workflow/catalog paths.
- Git failure and publish failure now both have executed proof for structured `delivery_receipt` persistence.
- Lineage-gated terminal backfill is implemented and metadata-only backfill is explicitly rejected.
- Northbound readback exists for frozen delivery config and release evidence.
- The repo-owned `proposal-045` gate exists and passes on the audited tree.

### Divergences

- The remaining proof gaps are now edge-case specific rather than core-path specific: missing-config fail-close and preserve-without-overwrite still rely primarily on direct code evidence.

### Ambiguities / Evidence Gaps

- I did not find a focused runtime test that drives the missing-`delivery_configuration_json` executor branch and then asserts “clear failure, no receipt written.”
- I did not find a focused runtime test that proves state_12 / later receipt writers preserve an already-existing `delivery_receipt` artifact without overwrite.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 9 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Frozen `delivery_configuration_json` input path is wired end-to-end
- Proposal Source: Scope; `§2a`; AC-1
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/commands.rs:10-28`
  - `control-plane/crates/engine/src/command_handler.rs:96-146`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:11-106`
  - `control-plane/crates/graphql-server/src/schema.rs:98-129`
  - `control-plane/crates/engine/tests/integration.rs:331-363`
  - `control-plane/crates/graphql-server/src/schema.rs:428-487`
  - `control-plane/crates/mcp-server/src/server.rs:496-524`
  - `./scripts/test-gate.sh proposal-045` (passed)
- Gap / Note: Frozen delivery config is accepted at both start surfaces, persisted on `Run`, and exposed on northbound readback.

### REQ-002 Native Rust release modules are present and exported
- Proposal Source: Scope; `§2b-§2e`; `§3`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/lib.rs:1-8`
  - `control-plane/crates/engine/src/release/mod.rs:1-7`
  - `control-plane/crates/engine/src/release/coordinator.rs:1-87`
  - `control-plane/crates/engine/src/release/git.rs:1-176`
  - `control-plane/crates/engine/src/release/connect.rs:1-124`
  - `control-plane/crates/engine/src/release/receipt.rs:1-77`
- Gap / Note: The coordinator remains available as the proposal requires even though live execution uses split per-agent routing.

### REQ-003 Release agents bypass ACP and execute natively per-agent
- Proposal Source: Goal; `§2f`; AC-2; AC-3; AC-12
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:462-779`
  - `control-plane/crates/engine/tests/release.rs:338-462`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `background_executor_routes_release_agents_natively`)
- Gap / Note: Release agents now definitively bypass ACP in executed proof, not just in code.

### REQ-004 Git release enforces deterministic branch safety and produces traceable push artifacts
- Proposal Source: `§2c`; `§4` constraints 1-3, 7; AC-2; AC-5
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/release/git.rs:43-133`
  - `control-plane/crates/engine/src/executor.rs:501-516`
  - `control-plane/crates/engine/tests/release.rs:179-223`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `git_release_service_commits_and_pushes_to_expected_branch` and `git_release_service_rejects_main_and_master_targets`)
- Gap / Note: Traceable commit metadata plus protected-branch rejection are now both under executed proof.

### REQ-005 Publish step consumes prior git artifacts and stays in sandbox/staging safe mode
- Proposal Source: `§2d`; `§4` constraint 8; AC-3; AC-13
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/release/connect.rs:43-111`
  - `control-plane/crates/engine/src/executor.rs:626-751`
  - `control-plane/crates/engine/tests/release.rs:226-278`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `connect_publish_service_creates_receipts_without_failing_on_missing_xcodeproj`)
- Gap / Note: Publish consumes git lineage, rejects unsupported release modes, and records safe-mode receipts.

### REQ-006 Canonical release artifact paths are used for transition truth, including `delivery_receipt`
- Proposal Source: `§2c` step 7; `§2f` steps 3/5/6; AC-4; AC-6; AC-10
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/agents/agents.yaml:49-53`
  - `control-plane/crates/engine/src/executor.rs:822-855`
  - `control-plane/crates/engine/src/executor.rs:896-910`
  - `control-plane/crates/engine/tests/release.rs:338-462`
  - `./scripts/test-gate.sh proposal-045` (passed)
- Gap / Note: `delivery_receipt` remains on canonical `.chainworks/release/delivery-receipt.json`; the old R2 blocker is still definitively closed.

### REQ-007 Delivery receipt persistence, release-attempt failure semantics, preserve-vs-backfill, and lineage-gated backfill are implemented
- Proposal Source: `§2e`; `§2g`; AC-4; AC-7; AC-8; AC-9; AC-10; AC-11
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/release/receipt.rs:29-76`
  - `control-plane/crates/engine/src/executor.rs:562-582`
  - `control-plane/crates/engine/src/executor.rs:656-676`
  - `control-plane/crates/engine/src/executor.rs:721-741`
  - `control-plane/crates/engine/src/executor.rs:857-953`
  - `control-plane/crates/engine/tests/release.rs:280-337`
  - `control-plane/crates/engine/tests/release.rs:468-659`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `background_executor_persists_delivery_receipt_on_git_failure`, `background_executor_persists_delivery_receipt_on_publish_failure`, `advance_run_backfills_delivery_receipt_when_terminal_release_lineage_exists`, and `advance_run_does_not_backfill_delivery_receipt_without_release_lineage`)
- Gap / Note: This is the main delta from `R3`: both executor-side release-attempt failure receipt branches are now directly executed and asserted.

### REQ-008 Northbound readback exposes frozen delivery config and release evidence
- Proposal Source: Scope B; gate scope (“northbound readback for frozen delivery config and release evidence”)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:428-487`
  - `control-plane/crates/mcp-server/src/server.rs:496-524`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:145-178`
  - `./scripts/test-gate.sh proposal-045` (passed; includes GraphQL and MCP tests)
- Gap / Note: Both frozen config and release evidence remain visible through current northbound read surfaces.

### REQ-009 The proposal-owned `proposal-045` gate exists and passes on the audited tree
- Proposal Source: `§6 Test Gate`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:521-538`
  - `scripts/test-gate.sh:1478-1487`
  - `./scripts/test-gate.sh proposal-045` (passed on `HEAD ddc5c0d`)
- Gap / Note: For this separate Rust control-plane slice, `proposal-045` remains the canonical same-tree sign-off lane.

## Architecture Review

**Summary:** Acceptable

- No architecture-level blocker remains. The split between per-agent executor routing, reusable release services, and lineage-gated receipt backfill is coherent and now better exercised.

## Product Review

**Summary:** Acceptable

- The deterministic release job promised by P045 is now covered on happy path, git failure, publish failure, and lineage-gated backfill paths.

## UI Review

**Summary:** Acceptable

- No direct UI findings. This proposal does not introduce UI surfaces.

## UX Review

**Summary:** Acceptable

- No operator-flow blocker remains. The UX contract in scope is deterministic safety and evidence continuity, and the current proof basis now covers the important failure paths too.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Remaining proof gaps are now edge-case-specific
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-007
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:791-792`
  - `control-plane/crates/engine/src/executor.rs:878-880`
  - `control-plane/crates/engine/tests/release.rs:468-659`
  - `./scripts/test-gate.sh proposal-045` (passed)
- Why It Matters: The core release path is now strongly exercised, including structured receipts on both release-attempt failure branches. What remains unexecuted are narrower safety edges: explicit missing-config fail-close with no receipt, and preserve-without-overwrite when an existing `delivery_receipt` is already present. Those are smaller risks than in `R3`, but they are still not zero.
- Recommended Action: Add one focused test for missing `delivery_configuration_json` fail-close and one focused test proving an existing `delivery_receipt` survives later write sites unchanged.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-045` passed on `HEAD ddc5c0d` |
| Core user flow runtime-validated | Pass | Proposal gate passed, and core happy/failure/backfill slices are all under executed proof |
| Empty/loading/error states covered | Not Applicable | No direct UI surface in scope |
| Accessibility risk acceptable | Not Applicable | No direct UI surface in scope |
| Localization risk acceptable | Not Applicable | No direct UI surface in scope |
| Critical tests executed | Pass | Gate executed engine integration/release tests plus GraphQL and MCP proof surfaces |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-045` passed and is the canonical sign-off gate for this control-plane slice |
| Privacy/permissions/entitlements reviewed | Not Applicable | No new entitlement or permission surface in scope |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/045-deterministic-release-operations.md`
- `git rev-parse --short HEAD`
- `python3 - <<'PY' ... git status --short summary ... PY`
- `sed -n '1,260p' docs/proposals/045-deterministic-release-operations_IMPLEMENTATION_AUDIT_R3.md`
- `./scripts/test-gate.sh proposal-045`
- `rg -n "background_executor_persists_delivery_receipt_on_git_failure|background_executor_persists_delivery_receipt_on_publish_failure" control-plane/crates/engine/tests/release.rs -S`
- `sed -n '450,900p' control-plane/crates/engine/tests/release.rs`
- `sed -n '1474,1490p' scripts/test-gate.sh`
- `sed -n '521,538p' docs/reference/test-gates.md`

## Recommended Next Actions

1. Add a focused missing-config fail-close test that proves no executor-side `delivery_receipt` is written.
2. Add a focused preserve-without-overwrite test so later write sites cannot regress receipt stability silently.
