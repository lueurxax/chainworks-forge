# Proposal 045: Deterministic Release Operations (Git + Sandbox Publish) Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | docs/proposals/045-deterministic-release-operations.md |
| Repository Root | . |
| Git SHA | ddc5c0d |
| Working Tree | dirty (0 index, 4717 worktree, 100912 untracked, 0 ignored) |
| Audited At | 2026-04-15T08:59:24+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready with Risks |
| Audit Confidence | High |

## Executive Verdict

P045 is now implemented on the audited tree. The old R2 blocker about `delivery_receipt` lacking a canonical path is closed: the artifact map now defines `delivery_receipt`, executor persistence resolves it through `plan.artifact_paths`, and the proposal-owned `proposal-045` gate passes on the same `HEAD`. The remaining limitation is proof shape rather than missing substrate. The happy path, canonical pathing, northbound readback, and lineage-gated backfill are all executed, but the executor's release-attempt failure receipt write sites are still stronger in code than in focused runtime proof, so readiness is `Ready with Risks` rather than a zero-risk sign-off.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | Failure-path receipt write sites are mostly code-proved rather than directly executed | High |
| Architecture | Acceptable | Release truth spans executor routing, artifact-path resolution, and receipt backfill logic | High |
| Product | Acceptable | Deterministic git/publish flow works, but executor-side failure-path evidence is thinner than happy-path evidence | Medium |
| UI | Acceptable | No proposal-specific UI surface is in scope | Medium |
| UX | Acceptable | No new operator interaction surface is introduced by this proposal | Medium |
| Readiness | Ready with Risks | Publish-failure and pre-release failure branches are not isolated by dedicated executed focused tests | High |

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
4. Reach terminal readback surfaces with frozen delivery config and release evidence available northbound.
5. On release-attempt or terminal backfill paths, preserve structured release truth without metadata-only receipt synthesis.

### UI Commitments

- No direct UI commitments. The proposal is daemon/release-path implementation only.

### UX Commitments

- Release execution is deterministic and non-LLM.
- Missing config/worktree fails closed.
- Failure paths preserve structured release truth instead of silently synthesizing success-like metadata.
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
- Git, publish, and delivery receipt artifacts resolve through canonical workflow/catalog paths, including `delivery_receipt`.
- Lineage-gated terminal backfill is implemented and metadata-only backfill is explicitly rejected.
- Northbound readback exists for frozen delivery config and release evidence.
- The repo-owned `proposal-045` gate exists and passes on the audited tree.

### Divergences

- The current focused proof corpus is richer than the original proposal sketch in some places and thinner in others: happy-path and backfill paths are executed directly, but executor-side failure receipt write sites are not isolated by named tests.

### Ambiguities / Evidence Gaps

- I did not find a focused runtime test that directly executes the git-failure or publish-failure executor branches and then asserts the exact persisted `delivery_receipt` payload.
- `ReleaseOpsCoordinator::execute_release()` exists and matches the proposal’s partial-failure contract, but the current executed proof is centered on per-agent executor routing rather than the coordinator facade itself.

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
- Gap / Note: The frozen config is now accepted at both northbound start surfaces, persisted on `Run`, and exposed on readback.

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
- Gap / Note: The coordinator exists as the proposal requires, even though the executor intentionally uses split per-agent routing for P044/P045 compatibility.

### REQ-003 Release agents bypass ACP and execute natively per-agent
- Proposal Source: Goal; `§2f`; AC-2; AC-3; AC-12
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:462-779`
  - `control-plane/crates/engine/tests/release.rs:338-462`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `background_executor_routes_release_agents_natively`)
- Gap / Note: This closes the original architecture seam from P044/P045 review: release agents no longer flow through ACP.

### REQ-004 Git release enforces deterministic branch safety and produces traceable push artifacts
- Proposal Source: `§2c`; `§4` constraints 1-3, 7; AC-2; AC-5
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/release/git.rs:43-133`
  - `control-plane/crates/engine/src/executor.rs:501-516`
  - `control-plane/crates/engine/tests/release.rs:179-223`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `git_release_service_commits_and_pushes_to_expected_branch` and `git_release_service_rejects_main_and_master_targets`)
- Gap / Note: The executor also includes the proposal-required traceable commit message with run ID and idea title before calling the git service.

### REQ-005 Publish step consumes prior git artifacts and stays in sandbox/staging safe mode
- Proposal Source: `§2d`; `§4` constraint 8; AC-3; AC-13
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/release/connect.rs:43-111`
  - `control-plane/crates/engine/src/executor.rs:626-751`
  - `control-plane/crates/engine/tests/release.rs:226-278`
  - `./scripts/test-gate.sh proposal-045` (passed; includes `connect_publish_service_creates_receipts_without_failing_on_missing_xcodeproj`)
- Gap / Note: The publish service explicitly rejects non-`sandbox`/`staging` modes and records safe-mode receipt output without real App Store Connect communication.

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
- Gap / Note: This is the main delta from R2. `delivery_receipt` now has a catalog-defined canonical path and is resolved through `plan.artifact_paths` instead of falling back to `artifact_root`.

### REQ-007 Delivery receipt persistence, preserve-vs-backfill, and lineage-gated backfill semantics are implemented
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
  - `control-plane/crates/engine/tests/release.rs:464-656`
  - `./scripts/test-gate.sh proposal-045` (passed; includes receipt-builder and backfill tests)
- Gap / Note: Happy-path and lineage-gated backfill are executed. The release-attempt failure write sites are proven strongly by code, but not by dedicated executed failure tests.

### REQ-008 Northbound readback exposes frozen delivery config and release evidence
- Proposal Source: Scope B; gate scope (“northbound readback for frozen delivery config and release evidence”)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:428-487`
  - `control-plane/crates/mcp-server/src/server.rs:496-524`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:145-178`
  - `./scripts/test-gate.sh proposal-045` (passed; includes GraphQL and MCP tests)
- Gap / Note: Both frozen config and release artifacts are now visible through current northbound read surfaces.

### REQ-009 The proposal-owned `proposal-045` gate exists and passes on the audited tree
- Proposal Source: `§6 Test Gate`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:521-538`
  - `scripts/test-gate.sh:1478-1487`
  - `./scripts/test-gate.sh proposal-045` (passed on `HEAD ddc5c0d`)
- Gap / Note: For this separate Rust control-plane slice, `proposal-045` is the canonical same-tree sign-off gate. The old R2 fail-close against the repo-wide `full` gate was stale.

## Architecture Review

**Summary:** Acceptable

- No architecture-level blocker remains. The current split between per-agent executor routing and the reusable coordinator is coherent and matches the proposal text.

## Product Review

**Summary:** Acceptable

- The deterministic release job promised by P045 is materially available: frozen config enters the run, native git/publish steps execute, canonical artifacts are produced, and northbound evidence is present.

## UI Review

**Summary:** Acceptable

- No direct UI findings. This proposal does not introduce new UI surfaces.

## UX Review

**Summary:** Acceptable

- No operator-flow blocker remains. The relevant UX commitments are deterministic safety and evidence continuity, both of which are substantially implemented.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Failure-path receipt write sites are more code-proved than runtime-proved
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-007
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:562-582`
  - `control-plane/crates/engine/src/executor.rs:721-741`
  - `control-plane/crates/engine/tests/release.rs:280-337`
  - `control-plane/crates/engine/tests/release.rs:464-656`
  - `./scripts/test-gate.sh proposal-045` (passed)
- Why It Matters: The executor clearly writes structured `delivery_receipt` payloads on git failure and publish failure, and the builder/backfill rules are executed. But the current test corpus still does not isolate those two executor-side failure branches as direct runtime proofs. That leaves a smaller, but real, regression-detection gap in the most safety-sensitive failure paths.
- Recommended Action: Add two focused tests: one for git-step failure proving structured failed `delivery_receipt` plus no publish artifacts, and one for publish-step failure proving preserved git artifacts plus structured failed `delivery_receipt`.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-045` passed on `HEAD ddc5c0d` |
| Core user flow runtime-validated | Pass | Proposal gate passed, and `cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --nocapture` also passed on the same tree |
| Empty/loading/error states covered | Not Applicable | No direct UI surface in scope |
| Accessibility risk acceptable | Not Applicable | No direct UI surface in scope |
| Localization risk acceptable | Not Applicable | No direct UI surface in scope |
| Critical tests executed | Pass | Gate executed engine integration/release tests plus GraphQL and MCP proof surfaces |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-045` passed and is the canonical sign-off gate for this control-plane slice |
| Privacy/permissions/entitlements reviewed | Not Applicable | No new app entitlement or permission surface in scope |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/045-deterministic-release-operations.md`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD`
- `python3 - <<'PY' ... git status --short summary ... PY`
- `sed -n '1,520p' docs/proposals/045-deterministic-release-operations.md`
- `rg -n "proposal-045|p045|Deterministic release" scripts/test-gate.sh docs/reference/test-gates.md -S`
- `rg -n "delivery_receipt|resolve_release_artifact_path|persist_delivery_receipt_if_absent|backfill_delivery_receipt_if_eligible" control-plane/crates/engine examples -S`
- `sed -n '1,220p' control-plane/crates/domain/src/commands.rs`
- `sed -n '80,165p' control-plane/crates/engine/src/command_handler.rs`
- `sed -n '1,150p' control-plane/crates/mcp-server/src/tools/runs.rs`
- `sed -n '90,155p' control-plane/crates/graphql-server/src/schema.rs`
- `sed -n '1,260p' control-plane/crates/engine/src/release/git.rs`
- `sed -n '1,260p' control-plane/crates/engine/src/release/connect.rs`
- `sed -n '1,220p' control-plane/crates/engine/src/release/receipt.rs`
- `sed -n '500,980p' control-plane/crates/engine/src/executor.rs`
- `sed -n '1,760p' control-plane/crates/engine/tests/release.rs`
- `./scripts/test-gate.sh proposal-045`
- `cd control-plane && cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --nocapture`

## Recommended Next Actions

1. Add direct runtime proofs for git-failure and publish-failure executor branches so the structured failure receipt semantics are not only code-inspected.
