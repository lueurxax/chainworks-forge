# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R9

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T17:20:46+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 007 remains `Partial`, but `R9` is materially stronger than `R8`. The updated repository documentation now clearly defines what counts as canonical proof: UI work is remote-only, agent gate proof should go through `./scripts/test-gate.sh ...`, raw `xcodebuild -testPlan ...` is diagnostic-only when it executes `0` tests, and proposal-level sign-off requires app-launched dogfood proof with real run-storage artifacts plus an exported evidence pack. Against that clarified contract, the same-`HEAD` sample-repo dogfood artifacts in `/tmp/p007-r6-sample-happy` and `/tmp/p007-r6-sample-nonhappy` now count as strong implementation evidence for the repo-backed delivery path. The proposal still does not reach `Implemented`, because the live review/refine loop is not directly re-proven in this audit round and the canonical gate path is still not green: `./scripts/test-gate.sh build` and `./scripts/test-gate.sh fast` both refused to start on this host due active app/test processes, while diagnostic direct `xcodebuild` runs still exposed a real compile regression in `ProviderPlatformTests`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | General sign-off gate is still not re-closed through the canonical agent gate path | High |
| Architecture | Acceptable | No fresh architecture blocker surfaced; the old repo-identity mismatch looks closed | High |
| Product | Acceptable | Same-`HEAD` happy-path and non-happy-path app-launched dogfood proof exists with real run-storage and exported evidence | High |
| UI | Evidence Gap | Fresh UI proof was not re-run in this audit because local UI execution is disallowed and remote execution was already occupied | Medium |
| UX | Acceptable | The repo-backed operator journey is evidenced by real dogfood artifacts, but current-round UI reproduction was not refreshed | Medium |
| Readiness | Not Ready | Canonical gate execution is still blocked/unproven, and diagnostic direct tests still show unrelated compile debt | High |

## Proposal Contract

### Scope

- Deliver the first full repo-backed 12-state workflow from idea through completed release candidate inside the app.
- Freeze `DeliveryConfiguration` before execution and carry it through Start Run, preflight, run creation, and resume.
- Provision one dedicated writable worktree per run before the first implementation write.
- Keep release side effects explicit and deterministic.
- Export a dogfood evidence pack for both happy-path and non-happy-path repo-backed runs.

### Locked Decisions

- One run equals one dedicated writable worktree.
- No concurrent writable agents may share a writable worktree.
- Release mechanics execute through deterministic services, not free-form agent shelling.
- `full-mvp-live.yaml` remains a dedicated repo-backed preset.
- Approval gates remain explicit workflow states.
- Default release targets stay sandbox/staging, not production.
- Partial release failure returns to blocked/operator recovery instead of hidden rollback.

### Primary User Flows

1. Start a `Full MVP Live` repo-backed run with frozen delivery configuration and passing delivery preflight.
2. Approve the proposal and move into a dedicated worktree-backed implementation slice.
3. Review implementation, loop/refine as needed, then reach an explicit manual release gate.
4. Complete a happy-path or blocked-release dogfood session and export a reviewable evidence pack.

### UI Commitments

- Start Run supports the `Full MVP Live` preset and delivery-specific context.
- Run Progress exposes repo/worktree-aware progress.
- Release Gate View surfaces enough context for informed approval.
- Existing report/recovery/comparison surfaces continue working for repo-backed runs.

### UX Commitments

- Manual approval gates remain visible and explicit.
- Release failure returns to an operator-visible blocked state with preserved receipts.
- A single engineer can complete both happy-path and non-happy-path dogfood sessions without guessing.

### Acceptance Criteria

- Dedicated writable worktree is provisioned and persisted before the first implementation write.
- `full-mvp-live.yaml` compiles into a valid executable 12-state plan.
- Implementation review produces the required review artifacts.
- Manual release blocks on explicit approval.
- Deterministic release services emit durable receipts/manifests.
- Happy-path and non-happy-path repo-backed runs complete inside the app with exported evidence.
- `xcodebuild build && xcodebuild test` is green with no regressions.

### Test / Evidence Requirements

- One full happy-path dogfood run.
- One non-happy-path run.
- Exported evidence packs.
- Screenshot/review proof for release-gate/final-receipt review.
- Green canonical build/test proof.

### Explicit Exclusions

- Proposal 007 does not replace the operator-shell or provider-platform baselines.
- Proposal 007 does not introduce production release targets.
- Proposal 007 does not require a full repo browser or multi-repo orchestration surface.

## Proposal Fidelity / Divergence

### Matches

- `RunStartSnapshot` still freezes delivery truth and applies it atomically to `Run` at creation time.
- `RunRepository.createRunFromPlan(...)` remains the shared run-creation boundary for frozen delivery state.
- `Proposal007DogfoodHarness` still drives a real app-launched repo-backed run, exports an evidence pack, and persists a result JSON.
- Same-`HEAD` sample-repo dogfood runs exist for both `happy_path` and `non_happy_path`, and both contain real run-storage artifacts plus exported evidence packs.
- The old self-repo repository-identity mismatch no longer appears in the current code path because both the dogfood harness and the provisioner canonicalize identifiers through `RepositoryIdentityNormalizer`.

### Divergences

- The live implementation-review/refine contract is still not directly re-proven in this audit round; the exported evidence proves review artifacts, but not a fresh loop-back into another implementation iteration.
- The canonical agent proving path is now `./scripts/test-gate.sh ...`, and that path is still not green in this round because both `build` and `fast` refused to start on a busy host.
- Diagnostic direct `xcodebuild` test runs still expose a compile regression in `ProviderPlatformTests`, but those runs are supporting diagnostics only, not canonical gate proof.

### Ambiguities / Evidence Gaps

- Fresh UI proof was not re-run in this audit because the repository now treats UI execution as remote-only by default and the approved host was already occupied by operator-owned remote activity.
- The same-`HEAD` dogfood artifacts are strong proof for the repo-backed flow itself, but they do not replace the proposal's explicit green gate requirement.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 12 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state executable plan
- Proposal Source: `5. Canonical live workflow for Proposal 007`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/stage-summary.json`
- Gap / Note: The sample-repo happy-path dogfood run reached the expected repo-backed workflow stages.

### REQ-002 Start Run, delivery preflight, run creation, and resume share one frozen `DeliveryConfiguration`
- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/RunRepository.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/delivery-configuration.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/delivery-preflight.json`
- Gap / Note: The exported app-launched evidence pack proves frozen delivery state survives into the live run.

### REQ-003 One dedicated writable worktree is provisioned and persisted before the first implementation write
- Proposal Source: `7.1 Core rule`, `7.3 Persisted metadata`, `14. Acceptance criteria / Runtime / worktree`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/run-metadata.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/run-metadata.json`
- Gap / Note: Both app-launched runs persisted real `workspaceRoot` / `artifactRoot` paths under run storage before terminal completion.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree
- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`, `14. Acceptance criteria / Runtime / worktree`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Engine/Proposal007DogfoodHarness.swift:189-205`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-95`
  - `Chainworks Forge/Engine/DeliveryConfiguration.swift:1-42`
  - `/tmp/p007-r6-sample-happy/result.json`
- Gap / Note: The old identifier-form mismatch no longer appears in the code path that produces and validates delivery configuration.

### REQ-005 Approved implementation agents execute against the real provisioned worktree with explicit source context
- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/diff-summary.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/stage-summary.json`
- Gap / Note: The happy-path app-launched proof shows real provisioned-worktree execution and downstream delivery artifacts.

### REQ-006 The implementation review/refine loop persists required artifacts and can iterate until `Implemented`
- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`
- Status: Partially Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/docs-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/audit-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/security-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/prepush-review-report.json`
- Gap / Note: The required review artifacts are proven, but this audit did not directly prove a fresh runtime loop-back into another implementation pass after review.

### REQ-007 Manual release remains explicit and a dedicated release-gate surface exists
- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
- Gap / Note: Both app-launched runs recorded explicit approvals and terminal release-state behavior.

### REQ-008 Release side effects execute only through deterministic runtime services
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract`, `15. Locked decisions / ARCH-069`
- Status: Implemented
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/delivery-receipt.json`
- Gap / Note: The live dogfood packets include deterministic release-side receipts/manifests rather than free-form shell evidence.

### REQ-009 Commit/push emits `git_push_receipt` and `release_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`, `14. Acceptance criteria / Release`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/git-push-receipt.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/release-manifest.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/git-push-receipt.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/release-manifest.json`
- Gap / Note: Both happy-path and non-happy-path dogfood packets include the expected Git deliverables.

### REQ-010 Archive/distribute emits `connect_upload_receipt` and `release_bundle_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`, `14. Acceptance criteria / Release`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/connect-upload-receipt.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/release-bundle-manifest.json`
- Gap / Note: The happy-path dogfood packet proves the archive/distribute deliverables on the real app-launched path.

### REQ-011 Partial release failure preserves receipts and returns the run to blocked/operator recovery
- Proposal Source: `9.4 Partial failure semantics`, `14. Acceptance criteria / Release and Dogfooding`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/delivery-receipt.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/git-push-receipt.json`
- Gap / Note: The non-happy-path run ends blocked with preserved partial delivery receipts and no connect-upload receipt.

### REQ-012 The app exports a dogfood evidence pack with the promised delivery artifacts
- Proposal Source: `12.2 Evidence pack builder`, `14. Acceptance criteria / Dogfooding`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
- Gap / Note: Both dogfood runs exported full evidence packs with delivery configuration, preflight, stage summary, agent detail, and deliverables.

### REQ-013 Happy-path and non-happy-path repo-backed runs complete from inside the app with exported evidence
- Proposal Source: `12.3 Manual dogfood script`, `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
- Gap / Note: These are same-`HEAD` app-launched dogfood artifacts with real run-storage paths and exported evidence packs, which now matches the repository's documented proposal-level proof contract.

### REQ-014 `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices
- Proposal Source: `14. Acceptance criteria / General`
- Status: Partially Implemented
- Evidence Type: tests-run, runtime
- Evidence:
  - `./scripts/test-gate.sh build`
  - `./scripts/test-gate.sh fast`
  - `/tmp/p007-r8-build.xcresult`
  - `/tmp/p007-r8-fast.xcresult`
  - `/tmp/p007-r8-unit.xcresult`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1380-1385`
  - `Chainworks Forge/Support/AppConfiguration.swift:17-30`
- Gap / Note: The canonical agent gate path is not green in this round. `test-gate.sh build` and `fast` both refused to start on a busy host, and diagnostic direct `xcodebuild` test runs still show a real compile regression in `ProviderPlatformTests` before any tests execute.

## Architecture Review

**Summary:** Acceptable

No fresh architecture finding is asserted in this round. The major `R8` architecture concern is now closed: the delivery identity contract looks coherent across the dogfood harness and the provisioner.

## Product Review

**Summary:** Acceptable

No fresh product blocker is asserted in this round. The same-`HEAD` happy-path and non-happy-path app-launched proof is strong enough to show that the repo-backed delivery slice is real, not aspirational.

## UI Review

**Summary:** Evidence Gap

No new UI-only defect is asserted in this round. Fresh UI proof was not re-run because the repository now treats UI execution as remote-only by default, and the current audit did not take over the already-occupied remote host path.

## UX Review

**Summary:** Acceptable

No fresh UX blocker is asserted in this round. The operator journey is materially evidenced by the live dogfood packets, even though the UI proof path itself was not refreshed during this audit session.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The canonical gate runner is still not closing Proposal 007 sign-off
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-014`
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh build`
  - `./scripts/test-gate.sh fast`
- Why It Matters: The repository documentation now makes `./scripts/test-gate.sh ...` the canonical proving path for agents. In this round, both relevant gates refused to start because the host already had active app/debug/test processes. That means the proposal's sign-off gate is still not freshly closed through the accepted mechanism.
- Recommended Action: Re-run the canonical gate path on a clean host state, or on the approved remote machine, once the current operator-owned processes are no longer active.

### READY-002 Diagnostic direct tests still expose unrelated compile debt behind the gate
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-014`
- Evidence Type: tests-run, code
- Evidence:
  - `/tmp/p007-r8-fast.xcresult`
  - `/tmp/p007-r8-unit.xcresult`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1380-1385`
  - `Chainworks Forge/Support/AppConfiguration.swift:17-30`
- Why It Matters: Direct `xcodebuild` test runs are no longer canonical proof, but they still provide useful diagnostics. Right now those diagnostics show a real regression: `ProviderPlatformTests` still constructs `AppConfiguration()` even though the type no longer has a zero-argument initializer. That debt will remain a likely gate blocker even after the host is cleaned up.
- Recommended Action: Fix `ProviderPlatformTests` to use `AppConfiguration.seededDefault()` or an explicit fixture before the next canonical gate rerun.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Diagnostic direct build was green at `/tmp/p007-r8-build.xcresult`, but the canonical gate runner build path did not execute on this busy host. |
| Core user flow runtime-validated | Pass | Same-`HEAD` app-launched happy-path and non-happy-path repo-backed proof exists with real run-storage and exported evidence packs. |
| Empty/loading/error states covered | Partial | Non-happy-path blocked delivery is proven through dogfood artifacts; fresh UI-smoke state coverage was not rerun. |
| Accessibility risk acceptable | Not Checked | No fresh UI run was executed in this audit session. |
| Localization risk acceptable | Not Checked | No localization-specific validation ran in this audit. |
| Critical tests executed | Partial | Canonical gates did not start; diagnostic direct tests ran and exposed compile debt before any tests executed. |
| Privacy/permissions/entitlements reviewed | Partial | Remote-only policy is explicit, but this audit did not freshly exercise the approved remote UI path. |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD && git rev-parse HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `md5 -q docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `sed -n '1,260p' docs/reference/agent-ui-test-execution.md`
- `sed -n '1,260p' scripts/test-gate.sh`
- `./scripts/test-gate.sh build`
- `./scripts/test-gate.sh fast`
- `sed -n '1,220p' /tmp/p007-r6-sample-happy/result.json`
- `sed -n '1,220p' /tmp/p007-r6-sample-nonhappy/result.json`
- `find /tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974 -maxdepth 2 -type f | sort`
- `find /tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9 -maxdepth 2 -type f | sort`
- `xcrun xcresulttool get build-results summary --path '/tmp/p007-r8-fast.xcresult'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p007-r8-fast.xcresult'`
- `xcrun xcresulttool get build-results summary --path '/tmp/p007-r8-unit.xcresult'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p007-r8-unit.xcresult'`

## Recommended Next Actions

1. Fix the `ProviderPlatformTests` compile regression so the next gate rerun is not blocked by unrelated test-target debt.
2. Re-run `./scripts/test-gate.sh build` and the relevant non-UI gate on a clean host state, or on the approved remote machine, once the current processes are no longer active.
3. After the canonical gates are green, refresh one approved-host UI proof or app-driven check only if needed to update operator-facing confidence, not to rediscover already-proven repo-backed runtime behavior.
