# Proposal 045: Deterministic Release Operations (Git + Sandbox Publish) Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/045-deterministic-release-operations.md` |
| Repository Root | `.` |
| Git SHA | `ddc5c0d52aff` |
| Working Tree | dirty (many modified and untracked files already present before this audit pass) |
| Audited At | `2026-04-15T08:03:22+03:00` |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 045 is materially landed but not closed on the current tree. The frozen `delivery_configuration_json` input path exists end-to-end, native Rust release services are present, release agents bypass ACP, and focused release proofs are green. The audit still fails closed on two explicit proposal contracts: release artifacts are not materialized at the canonical workflow paths needed for `exists('git_push_receipt')`, and the repo-owned `proposal-045` proof lane promised by the proposal does not exist in the repository. That keeps the slice at `Not Implemented` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Canonical artifact-path transition contract is still missing | High |
| Architecture | At Risk | Release agents use a separate artifact persistence path from the canonical workflow path model | High |
| Product | At Risk | A successful native git push can still fail to satisfy the workflow transition contract | High |
| UI | Acceptable | No proposal-specific UI surface is directly in scope | Medium |
| UX | Acceptable | No proposal-specific operator interaction redesign is directly in scope | Medium |
| Readiness | Not Ready | The repo-owned `proposal-045` gate is still absent | High |

## Proposal Contract

### Scope

- Port Swift `GitReleaseService`, `ConnectPublishService`, `ReleaseOpsCoordinator`, and `DeliveryReceiptBuilder` into the Rust daemon.
- Add `delivery_configuration_json` through `StartRunCmd`, command handling, and northbound start surfaces.
- Keep release execution deterministic and native, not ACP-driven.

### Locked Decisions

- Release agents execute through native Rust services, not ACP.
- `delivery_configuration_json` is frozen at run start and deserialized fail-closed at release time.
- Git and publish run as separate per-agent steps.
- `delivery_receipt.json` is preserved on happy paths and release-attempt failure paths, with terminal backfill only when prior release-agent lineage can produce release truth.
- Release remains sandbox/staging safe mode only; no real App Store Connect communication.
- Proof ownership is repo-owned through a `proposal-045` gate and a matching `test-gates.md` entry.

### Primary User Flows

1. Start a repo-backed run with frozen `delivery_configuration_json`, persist it on the run, and make it available to release execution.
2. Execute `commit_and_push_to_github` natively, produce git release artifacts, and record traceable commit metadata.
3. Execute `build_archive_and_push_connect` natively using prior git artifacts, produce publish artifacts, and persist `delivery_receipt.json`.
4. Fail git or publish deterministically, preserving structured release truth without ACP mediation or metadata-only receipt synthesis.

### UI Commitments

- No new UI surfaces are introduced by this proposal.
- Operator-facing proof lives in artifacts, workflow transitions, and repo-owned gate coverage.

### UX Commitments

- Release execution is deterministic and non-LLM.
- Missing config/worktree fails closed.
- Failure paths preserve structured delivery truth.
- Terminal backfill is lineage-gated rather than metadata-only.

### Acceptance Criteria

1. `delivery_configuration_json` accepted at MCP and GraphQL start surfaces, persisted on `Run`, and deserialized at release time.
2. `commit_and_push_to_github` produces `release_manifest.json` and `git_push_receipt.json` without ACP.
3. `build_archive_and_push_connect` consumes git artifacts and produces `release_bundle_manifest.json` and `connect_upload_receipt.json`.
4. `delivery_receipt.json` is persisted on happy paths and release-attempt failure paths.
5. Git push writes a real commit to the target branch.
6. Artifacts land at canonical paths so `exists('git_push_receipt')` evaluates true.
7. Git failure records structured release failure truth instead of `release_result: None`.
8. Publish failure preserves git artifacts and records structured release failure truth.
9. Missing `delivery_configuration_json` fails closed with no executor-side receipt on that pre-release path.
10. Preserve-vs-backfill semantics prevent overwriting an existing `delivery_receipt.json`.
11. State_12 backfill requires prior release-agent lineage.
12. No LLM participates in the release path.
13. Publish mode remains `sandbox` or `staging`, never production.

### Test / Evidence Requirements

- Add a repo-owned `proposal-045` lane in `scripts/test-gate.sh`.
- Add a matching `docs/reference/test-gates.md` entry.
- Cover input path, git release, sandbox publish, partial failure, delivery receipt behavior, native executor routing, and main-branch rejection.

### Explicit Exclusions

- Real App Store Connect upload.
- Production release mode.
- Broader post-approval orchestration beyond the adjacent `P044` contract.
- UI/CLI repo-profile selection.

## Proposal Fidelity / Divergence

### Matches

- `delivery_configuration_json` now exists on `StartRunCmd`, is persisted on `Run`, and is accepted by both MCP and GraphQL start surfaces.
- Rust-native release modules exist for git, publish, coordinator, and receipt building.
- Release agents are routed natively in the executor rather than through ACP.
- Structured delivery receipt persistence and lineage-gated backfill are implemented.
- Sandbox/staging safe-mode publish behavior is implemented and runtime-proved in focused tests.

### Divergences

- Release artifacts are still written to `artifact_root/{name}.json`, not the canonical workflow artifact paths promised by the proposal.
- The workflow transition contract `exists('git_push_receipt')` still depends on a canonical-path layout the release executor does not produce.
- The repo-owned `proposal-045` gate and `test-gates.md` entry are still absent.

### Ambiguities / Evidence Gaps

- No focused executed proof was found for the pre-release `delivery_configuration_json = None` failure path; that path is covered by direct code inspection rather than a dedicated test run in this audit.
- No same-tree full regression run was executed because the audit already failed on explicit missing requirements and the proposal-owned gate is absent.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 0 |
| Missing | 2 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Frozen `delivery_configuration_json` input path is wired end-to-end
- Proposal Source: Scope; §2a; AC-1
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/domain/src/commands.rs:16-28`
  - `control-plane/crates/engine/src/command_handler.rs:98-156`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:13-121`
  - `control-plane/crates/graphql-server/src/schema.rs:101-126`
  - `control-plane/crates/db/src/repos/runs.rs:23-55`
  - `control-plane/crates/engine/tests/integration.rs:331-363`
  - `cargo test -p engine --test integration test_start_run_persists_delivery_configuration_json -- --exact --nocapture` -> `1 passed`
- Gap / Note: This requirement is directly implemented and narrowly runtime-proved at the command-handler persistence seam.

### REQ-002 Native Rust release services are ported and exported
- Proposal Source: Scope; §2b-§2e; §3
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/lib.rs:1-8`
  - `control-plane/crates/engine/src/release/coordinator.rs:1-87`
  - `control-plane/crates/engine/src/release/git.rs:1-179`
  - `control-plane/crates/engine/src/release/connect.rs:1-124`
  - `control-plane/crates/engine/src/release/receipt.rs:1-77`
- Gap / Note: `ReleaseOpsCoordinator` exists as a native library path even though the executor intentionally uses split per-agent routing.

### REQ-003 Release agents bypass ACP and execute natively per-agent
- Proposal Source: Goal; §2f; AC-2; AC-3; AC-12
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:500-779`
  - `control-plane/crates/engine/tests/release.rs:305-408`
  - `cargo test -p engine --test release -- --nocapture` -> `6 passed`
- Gap / Note: The focused release suite proves both release agents execute without ACP adapters and produce the expected artifact set.

### REQ-004 Git release enforces branch safety and traceable commit metadata
- Proposal Source: §2c; §4.2; §4.3; §4.7; AC-5
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/release/git.rs:45-76`
  - `control-plane/crates/engine/src/executor.rs:500-505`
  - `control-plane/crates/engine/tests/release.rs:166-191`
  - `cargo test -p engine --test release -- --nocapture` -> `6 passed`
- Gap / Note: The code rejects pushes to `main` / `master`, requires the expected branch, and formats the commit message with run ID and idea title. This audit did not find a dedicated executed rejection test for `main` / `master`, but direct code evidence satisfies the implementation claim.

### REQ-005 Publish step consumes git artifacts and stays in sandbox/staging safe mode
- Proposal Source: §2d; §4.8; AC-3; AC-13
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/release/connect.rs:46-107`
  - `control-plane/crates/engine/tests/release.rs:193-245`
  - `cargo test -p engine --test release -- --nocapture` -> `6 passed`
- Gap / Note: The service requires upstream git artifacts, constrains release mode to `sandbox` / `staging`, and records safe-mode receipts without real App Store Connect communication.

### REQ-006 Delivery receipt persistence, preserve-if-absent semantics, and lineage-gated backfill are implemented
- Proposal Source: §2e; §2g; AC-4; AC-7; AC-8; AC-9; AC-10; AC-11
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/release/receipt.rs:33-77`
  - `control-plane/crates/engine/src/executor.rs:561-582`
  - `control-plane/crates/engine/src/executor.rs:720-741`
  - `control-plane/crates/engine/src/executor.rs:857-937`
  - `control-plane/crates/engine/src/executor.rs:939-980`
  - `control-plane/crates/engine/tests/release.rs:247-303`
  - `control-plane/crates/engine/tests/release.rs:410-520`
  - `cargo test -p engine --test release -- --nocapture` -> `6 passed`
- Gap / Note: The focused release suite proves both positive and negative backfill eligibility and confirms that metadata-only receipt synthesis is rejected without release lineage.

### REQ-007 Release artifacts are written to canonical workflow paths so `exists('git_push_receipt')` is true
- Proposal Source: §2c step 7; §2f steps 3 and 5; AC-6
- Status: Missing
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `docs/proposals/045-deterministic-release-operations.md:267-277`
  - `docs/proposals/045-deterministic-release-operations.md:338-344`
  - `examples/workflows/full-mvp-live.yaml:352-373`
  - `examples/agents/agents.yaml:49-52`
  - `control-plane/crates/engine/src/executor.rs:341-349`
  - `control-plane/crates/engine/src/executor.rs:822-855`
  - `control-plane/crates/engine/src/executor.rs:1028-1083`
  - `control-plane/crates/engine/src/orchestrator.rs:959-980`
  - `control-plane/crates/engine/tests/release.rs:390-407`
- Gap / Note: The release path writes `release_manifest.json`, `git_push_receipt.json`, and peers into `artifact_root`, not the catalog-defined canonical paths such as `.chainworks/release/git-push.json`. The ACP path has `normalize_artifacts(...)`, but the release path does not use it. `exists('git_push_receipt')` checks the canonical path first and then falls back to `artifact_root/git_push_receipt` without the `.json` suffix, so the proposal's transition-proof contract is not currently satisfied.

### REQ-008 Repo-owned `proposal-045` proof lane exists in `scripts/test-gate.sh` and `docs/reference/test-gates.md`
- Proposal Source: §6 Test Gate
- Status: Missing
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/045-deterministic-release-operations.md:354-405`
  - `scripts/test-gate.sh:1188-1196`
  - `scripts/test-gate.sh:1218-1495`
  - `docs/reference/test-gates.md:501-546`
  - `rg -n "proposal-045|p045|PROPOSAL_045_TESTS" scripts/test-gate.sh docs/reference/test-gates.md` -> exit `1` (no matches)
- Gap / Note: The repository currently defines `proposal-044` and then jumps directly to `full`. The proposal-promised gate inventory, case block, and canonical docs entry for `proposal-045` have not landed.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Release execution uses a parallel artifact persistence model instead of the canonical workflow-path model
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Goal; §2c; §2f; REQ-007
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:341-349`
  - `control-plane/crates/engine/src/executor.rs:822-855`
  - `control-plane/crates/engine/src/executor.rs:1028-1083`
  - `control-plane/crates/engine/src/orchestrator.rs:959-980`
- Why It Matters: ACP-backed agents already normalize artifact output onto canonical catalog paths, but release agents bypass that contract and persist directly into `artifact_root`. That creates two incompatible artifact-truth models inside the same workflow engine and is the direct cause of the `exists('git_push_receipt')` divergence.
- Recommended Action: Make native release writes resolve against `plan.artifact_paths` or reuse the same normalization path that ACP-backed agents use, including filename parity with the catalog (`git-push.json`, `bundle.json`, etc.).

## Product Review

**Summary:** At Risk

### PROD-001 A successful native git push can still fail to unlock the workflow's next state
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Goal; AC-6; REQ-007
- Evidence Type: `code`
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:371-373`
  - `examples/agents/agents.yaml:49-52`
  - `control-plane/crates/engine/src/executor.rs:822-855`
  - `control-plane/crates/engine/src/orchestrator.rs:966-980`
- Why It Matters: The operator-visible release truth is not just the git side effect; it is the workflow advancing deterministically into the finalization state. If the release step succeeds but the transition condition still cannot see `git_push_receipt` at the canonical path, the control-plane behavior diverges from the proposal's intended end-to-end flow.
- Recommended Action: Treat canonical path materialization as part of the release-agent success contract, then add a focused proof that `state_11_manual_release` advances on a real `git_push_receipt` produced by the native executor.

## UI Review

**Summary:** Acceptable

- No proposal-specific UI surface is directly in scope for this daemon-side release slice.

## UX Review

**Summary:** Acceptable

- No new operator interaction model is introduced here beyond artifact truth and workflow progression, which are already covered under the conformance and product findings.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The proposal-owned proof lane is still missing, so sign-off is not reproducible
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: §6; REQ-008
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `scripts/test-gate.sh:1188-1196`
  - `scripts/test-gate.sh:1218-1495`
  - `docs/reference/test-gates.md:501-546`
  - `cargo test -p engine --test integration test_start_run_persists_delivery_configuration_json -- --exact --nocapture` -> `1 passed`
  - `cargo test -p engine --test release -- --nocapture` -> `6 passed`
- Why It Matters: The focused Rust proof that does exist is ad hoc and reproducible only by reading this audit. The proposal explicitly requires a repo-owned `proposal-045` lane and canonical docs entry so future engineers can reprove the slice on the same tree without reconstructing the test set manually.
- Recommended Action: Add `PROPOSAL_045_TESTS`, `proposal-045|p045`, and the matching `docs/reference/test-gates.md` entry, then run that lane on the same tree before claiming proposal closure.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Focused Rust control-plane tests compile and pass; no full repo build or app sign-off was executed. |
| Core user flow runtime-validated | Partial | Native git/publish/receipt paths were exercised in `control-plane` release tests, but canonical transition truth was not runtime-proved because REQ-007 is still missing. |
| Empty/loading/error states covered | Partial | Git failure, publish failure, and lineage-gated backfill are represented in code and release tests; the pre-release missing-config path was code-reviewed but not executed in this audit. |
| Accessibility risk acceptable | Pass | Daemon-only slice; no new UI or accessibility surface. |
| Localization risk acceptable | Pass | No new operator-facing UI text is in scope. |
| Critical tests executed | Pass | Focused input-path and release suites were run on this tree. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | No `proposal-045` gate exists, and the audit already failed on explicit missing requirements. |
| Privacy/permissions/entitlements reviewed | Pass | Daemon-only slice; no new permission or entitlement surface was introduced by the inspected code paths. |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/045-deterministic-release-operations.md`
- `git rev-parse HEAD`
- `git status --short`
- `rg -n "proposal-045|p045|PROPOSAL_045_TESTS" scripts/test-gate.sh docs/reference/test-gates.md`
- `cargo test -p engine --test integration test_start_run_persists_delivery_configuration_json -- --exact --nocapture`
- `cargo test -p engine --test release -- --nocapture`

## Recommended Next Actions

1. Fix REQ-007 by making native release artifacts land on canonical catalog paths, or by reusing the existing artifact normalization path for release agents with filename parity.
2. Add the repo-owned `proposal-045` gate and `docs/reference/test-gates.md` entry promised by §6, then make that lane the canonical repro command for this slice.
3. After the two explicit proposal gaps are fixed, rerun the proposal-owned gate on the same tree and only then consider a higher readiness verdict.
