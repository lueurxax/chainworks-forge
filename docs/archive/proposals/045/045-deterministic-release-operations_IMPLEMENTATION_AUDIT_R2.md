# Proposal 045: Deterministic Release Operations (Git + Sandbox Publish) Multi-Lens Audit R2

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

Proposal 045 has materially advanced since the prior audit. The frozen `delivery_configuration_json` input path is wired end-to-end, native release services are present, canonical git/publish artifact paths are now used, northbound readback proofs are present, and the repo-owned `proposal-045` gate now exists and passes on this tree. The slice still fails closed for two reasons: one explicit proposal requirement remains unimplemented because `delivery_receipt` still has no catalog-defined canonical path and therefore falls back to `artifact_root`, and the repository's canonical `full` gate is remote-only on this host, so the audit cannot produce a successful roll-up even if the remaining implementation gap were closed.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | `delivery_receipt` still lacks a canonical repo-defined path | High |
| Architecture | At Risk | Release receipt persistence still straddles catalog-defined and fallback artifact models | High |
| Product | Acceptable | Core git/publish flow is deterministic, but receipt-path truth remains inconsistent | High |
| UI | Acceptable | No proposal-specific UI surface is directly in scope | Medium |
| UX | Acceptable | No new operator interaction model is directly in scope | Medium |
| Readiness | Not Ready | Same-tree `proposal-045` is green, but canonical `full` sign-off is unavailable on this host | High |

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
4. `delivery_receipt.json` is persisted at canonical path on happy paths and on release-attempt failure paths.
5. Git push writes a real commit to the target branch.
6. All artifacts written to canonical paths so `exists('git_push_receipt')` evaluates true.
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

- `delivery_configuration_json` exists on `StartRunCmd`, is persisted on `Run`, and is accepted by MCP and GraphQL start surfaces.
- Rust-native release modules exist for git, publish, coordinator, and receipt building.
- Release agents are routed natively in the executor rather than through ACP.
- Git, publish, and northbound readback proof surfaces exist and are green in the current `proposal-045` lane.
- Git/publish artifacts now resolve against catalog-defined canonical paths when workflow/catalog YAML paths are present.

### Divergences

- `delivery_receipt` still has no artifact entry in the canonical catalog map, so receipt persistence falls back to `artifact_root`.
- The proposal-owned gate is green, but its proof corpus still does not directly exercise publish-failure or pre-release no-receipt paths.

### Ambiguities / Evidence Gaps

- No same-tree `full` regression evidence is available on this host because `scripts/test-gate.sh full` is remote-only and fail-closes before execution.
- The current release suite proves happy-path canonical git/publish artifacts and backfill eligibility, but not the executor-side failure receipt write sites.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 1 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Frozen `delivery_configuration_json` input path is wired end-to-end
- Proposal Source: Scope; §2a; AC-1
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/domain/src/commands.rs:16-28`
  - `control-plane/crates/engine/src/command_handler.rs:98-156`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:11-122`
  - `control-plane/crates/graphql-server/src/schema.rs:101-139`
  - `control-plane/crates/engine/tests/integration.rs:331-363`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
- Gap / Note: The gate explicitly exercises the engine input-path test plus GraphQL and MCP persistence/readback tests.

### REQ-002 Native Rust release services are ported and exported
- Proposal Source: Scope; §2b-§2e; §3
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/lib.rs:1-8`
  - `control-plane/crates/engine/src/release/coordinator.rs:1-87`
  - `control-plane/crates/engine/src/release/git.rs:1-203`
  - `control-plane/crates/engine/src/release/connect.rs:1-124`
  - `control-plane/crates/engine/src/release/receipt.rs:1-77`
- Gap / Note: `ReleaseOpsCoordinator` exists as a native library path even though the executor intentionally uses split per-agent routing.

### REQ-003 Release agents bypass ACP and execute natively per-agent
- Proposal Source: Goal; §2f; AC-2; AC-3; AC-12
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:500-779`
  - `control-plane/crates/engine/tests/release.rs:337-459`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
- Gap / Note: The focused release suite proves both release agents execute without ACP adapters and produce the expected artifact set.

### REQ-004 Git release enforces deterministic branch safety and traceable commit metadata
- Proposal Source: §2c; §4.2; §4.3; §4.7; AC-5
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/release/git.rs:50-176`
  - `control-plane/crates/engine/src/executor.rs:500-505`
  - `control-plane/crates/engine/tests/release.rs:178-223`
  - `cargo test -p engine --test release git_release_service_rejects_main_and_master_targets -- --exact --nocapture` -> `1 passed`
- Gap / Note: The implementation still uses an explicitly supplied working directory instead of literal `git -C ...`, but it does not rely on ambient cwd and it satisfies the proposal's explicit-path safety intent.

### REQ-005 Publish step consumes git artifacts and stays in sandbox/staging safe mode
- Proposal Source: §2d; §4.8; AC-3; AC-13
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/release/connect.rs:46-107`
  - `control-plane/crates/engine/tests/release.rs:225-277`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
- Gap / Note: The service requires upstream git artifacts, constrains release mode to `sandbox` / `staging`, and records safe-mode receipts without real App Store Connect communication.

### REQ-006 Structured release truth and lineage-gated backfill are implemented
- Proposal Source: §2e; §2g; AC-7; AC-8; AC-9; AC-10; AC-11
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/release/receipt.rs:33-77`
  - `control-plane/crates/engine/src/executor.rs:561-582`
  - `control-plane/crates/engine/src/executor.rs:720-741`
  - `control-plane/crates/engine/src/executor.rs:857-953`
  - `control-plane/crates/engine/tests/release.rs:279-335`
  - `control-plane/crates/engine/tests/release.rs:462-654`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
- Gap / Note: Backfill eligibility and metadata-only rejection are runtime-proved. Executor-side failure handlers are present in code but not separately exercised by a dedicated failure-path test.

### REQ-007 Git/publish artifacts land on canonical workflow paths so transition truth is satisfied
- Proposal Source: §2c step 7; §2f steps 3 and 5; AC-6
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `examples/agents/agents.yaml:49-52`
  - `examples/workflows/full-mvp-live.yaml:352-373`
  - `control-plane/crates/engine/src/executor.rs:812-855`
  - `control-plane/crates/engine/src/executor.rs:896-910`
  - `control-plane/crates/engine/tests/release.rs:431-450`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
- Gap / Note: `resolve_release_artifact_path(...)` now routes git and publish artifacts through `plan.artifact_paths` when workflow/catalog paths are present, and the release suite asserts the canonical `.chainworks/release/*.json` locations.

### REQ-008 `delivery_receipt.json` is persisted at a canonical path on happy and release-attempt failure paths
- Proposal Source: §2e pseudocode; §2f steps 5 and 6; AC-4; AC-10
- Status: Missing
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/045-deterministic-release-operations.md:226-241`
  - `docs/proposals/045-deterministic-release-operations.md:341-348`
  - `examples/agents/agents.yaml:19-70`
  - `rg -n "delivery_receipt:" examples/agents/agents.yaml` -> exit `1` (no match)
  - `control-plane/crates/engine/src/executor.rs:878-910`
  - `control-plane/crates/engine/tests/release.rs:453-458`
- Gap / Note: The proposal explicitly anchors receipt persistence to `plan.artifact_paths.get("delivery_receipt")`, but the current catalog has no `delivery_receipt` artifact entry. `resolve_release_artifact_path(...)` therefore falls back to `artifact_root` for receipts. The happy-path test only asserts a `.ends_with("delivery_receipt.json")` suffix, not a canonical repo-defined location.

### REQ-009 Repo-owned `proposal-045` proof lane and `test-gates.md` entry are landed
- Proposal Source: §6 Test Gate
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `scripts/test-gate.sh:1196`
  - `scripts/test-gate.sh:1477-1486`
  - `docs/reference/test-gates.md:521-539`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
- Gap / Note: This closes the prior audit blocker about missing repo-owned proof ownership.

### REQ-010 Proposal-owned proof corpus covers the declared critical cases
- Proposal Source: §6 `PROPOSAL_045_TESTS`; Test / Evidence Requirements
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `docs/proposals/045-deterministic-release-operations.md:383-405`
  - `control-plane/crates/engine/tests/release.rs:178-223`
  - `control-plane/crates/engine/tests/release.rs:225-277`
  - `control-plane/crates/engine/tests/release.rs:337-459`
  - `control-plane/crates/engine/tests/release.rs:462-654`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
  - `rg -n "failure_stage.*build_archive_and_push|failure_stage.*commit_and_push|Invalid delivery_configuration_json|requires delivery_configuration_json" control-plane/crates/engine/tests/release.rs control-plane/crates/engine/tests/integration.rs` -> no focused failure-path tests found
- Gap / Note: The lane is green and covers happy-path canonical artifacts, backfill eligibility, branch rejection, and northbound input/readback. It still does not directly prove publish-failure artifact preservation or the pre-release no-receipt path that the proposal text calls out.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Receipt persistence still straddles canonical-path and fallback artifact models
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: §2e; AC-4; REQ-008
- Evidence Type: `code`
- Evidence:
  - `examples/agents/agents.yaml:19-70`
  - `control-plane/crates/engine/src/executor.rs:878-910`
- Why It Matters: Git/publish artifacts now participate in the catalog-defined path model, but `delivery_receipt` still depends on a fallback path. That leaves the release slice with two artifact-truth models inside the same executor and means the receipt does not yet satisfy the proposal's canonical-path contract.
- Recommended Action: Add `delivery_receipt` to the catalog `artifacts:` map and strengthen the release suite to assert the exact resolved canonical path.

## Product Review

**Summary:** Acceptable

### PROD-001 Core deterministic release flow is now present, but receipt-path truth is still inconsistent
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: Goal; AC-4; REQ-008
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/tests/release.rs:337-459`
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
  - `control-plane/crates/engine/src/executor.rs:878-910`
- Why It Matters: The operator-visible product outcome is substantially better than in the prior audit because the release path is deterministic and natively exercised end-to-end. The remaining inconsistency is narrow but still real: delivery receipts are not stored under the same canonical artifact policy as the other release artifacts.
- Recommended Action: Finish the receipt-path normalization and add one proof that reads the receipt from its resolved canonical path on a same-tree run.

## UI Review

**Summary:** Acceptable

- No proposal-specific UI surface is directly in scope for this daemon-side release slice.

## UX Review

**Summary:** Acceptable

- No new operator interaction model is directly in scope beyond artifact truth and workflow progression, which are already covered under conformance and readiness.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Canonical `full` regression sign-off is unavailable on this host
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Audit-policy successful-roll-up gate
- Evidence Type: `tests-run`
- Evidence:
  - `scripts/test-gate.sh:1488-1503`
  - `bash ./scripts/test-gate.sh full` -> exit `3`, `UI tests are remote-only and may not run on this host`
- Why It Matters: The proposal-specific gate is green, but the audit skill fail-closes successful outcomes without a same-tree full regression run. On this host, the repository's canonical `full` gate cannot be executed.
- Recommended Action: Re-run the audit on an approved remote UI host that can execute `./scripts/test-gate.sh full`, after closing the remaining receipt-path gap.

### READY-002 The green proposal lane still under-proves failure-path truth
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: §6; REQ-010
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `bash ./scripts/test-gate.sh proposal-045` -> passed
  - `rg -n "failure_stage.*build_archive_and_push|failure_stage.*commit_and_push|Invalid delivery_configuration_json|requires delivery_configuration_json" control-plane/crates/engine/tests/release.rs control-plane/crates/engine/tests/integration.rs` -> no focused failure-path tests found
- Why It Matters: The lane is now useful and reproducible, but it still leaves the two most proposal-sensitive non-happy paths proved only by code inspection.
- Recommended Action: Add explicit release tests for publish-failure artifact preservation and the pre-release missing-config no-receipt path, then keep them in the `proposal-045` lane.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Focused Rust control-plane tests and the proposal-specific control-plane gate pass; no app-wide/full repository sign-off is available on this host. |
| Core user flow runtime-validated | Pass | `proposal-045` gate passes, covering start-run input persistence, native release execution, canonical git/publish artifact paths, GraphQL readback, and MCP readback. |
| Empty/loading/error states covered | Partial | Happy-path, branch rejection, and backfill eligibility are executed; publish-failure and pre-release no-receipt paths are still code-inspected rather than directly executed. |
| Accessibility risk acceptable | Pass | Daemon-only slice; no new UI or accessibility surface. |
| Localization risk acceptable | Pass | No new operator-facing UI text is in scope. |
| Critical tests executed | Pass | `proposal-045` gate passed on this tree. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | `bash ./scripts/test-gate.sh full` fail-closed immediately because the lane is remote-only on this host. |
| Privacy/permissions/entitlements reviewed | Pass | Daemon-only slice; no new permission or entitlement surface was introduced by the inspected code paths. |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/045-deterministic-release-operations.md`
- `git rev-parse HEAD`
- `git status --short`
- `rg -n "proposal-045|p045|PROPOSAL_045_TESTS|git_push_receipt|release_manifest|normalize_artifacts|release_artifact_path" scripts/test-gate.sh docs/reference/test-gates.md control-plane/crates/engine/src control-plane/crates/engine/tests examples/agents/agents.yaml examples/workflows/full-mvp-live.yaml`
- `bash ./scripts/test-gate.sh proposal-045`
- `cargo test -p engine --test release git_release_service_rejects_main_and_master_targets -- --exact --nocapture`
- `cargo test -p graphql-server -- --nocapture`
- `cargo test -p mcp-server -- --nocapture`
- `bash ./scripts/test-gate.sh full`

## Recommended Next Actions

1. Add `delivery_receipt` to the canonical catalog `artifacts:` map and make the release suite assert its exact resolved path, not just the filename suffix.
2. Add focused failure-path tests for publish-failure receipt persistence and the pre-release missing-config no-receipt contract, then keep them inside `proposal-045`.
3. Re-run the audit on an approved remote UI host that can execute `./scripts/test-gate.sh full`, because the audit skill cannot produce a successful roll-up without same-tree full regression evidence.
