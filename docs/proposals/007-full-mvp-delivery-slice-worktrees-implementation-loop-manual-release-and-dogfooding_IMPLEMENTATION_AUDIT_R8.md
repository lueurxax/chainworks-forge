# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R8

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T17:01:38+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 007 remains `Partial` on the current tree. The implementation itself is stronger than in `R7`: the old self-repo repository-identity mismatch is no longer visible in code because both the dogfood harness and the provisioner now canonicalize through `RepositoryIdentityNormalizer`, the shared `DeliveryConfiguration` freeze still exists, deterministic release services are still wired, and the app still builds successfully on macOS. The proposal still does not reach `Implemented`, because the current verification surface regressed again: both direct non-UI `xcodebuild test` paths fail at compile time before any tests execute, the repository-supported `FastGate` run still ends with `0` executed tests for that reason, local UI proof was explicitly forbidden by the operator, remote-only UI proof could not be refreshed because `SMacBook.local` rejected SSH auth from this environment, and the default product export locations contain no fresh current-round delivery evidence packs.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Proposal-level dogfood proof is still not refreshed on the current dirty tree | High |
| Architecture | Acceptable | The old repo-identity mismatch appears fixed, but that improvement is not yet backed by fresh current-round dogfood proof | High |
| Product | At Risk | Same-SHA sample-repo proof exists only as inherited `/tmp` artifacts, not as fresh current-round product-default exports | High |
| UI | Evidence Gap | Local UI execution was forbidden and the approved remote host path was unreachable from this environment | High |
| UX | At Risk | Sign-off still depends on evidence paths the operator cannot currently refresh from the documented workflow | Medium |
| Readiness | Not Ready | Non-UI verification currently fails before any tests execute, so Proposal 007 cannot close its own sign-off gate | High |

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

## Proposal Fidelity / Divergence

### Matches

- `RunStartSnapshot` still freezes delivery truth and applies it atomically to `Run` at creation time.
- `RunRepository.createRunFromPlan(...)` remains the shared run-creation boundary for the frozen `DeliveryConfiguration`.
- `Proposal007DogfoodHarness.makeDeliveryConfiguration(...)` now canonicalizes `repoIdentifier` through `RepositoryIdentityNormalizer`.
- `WorktreeProvisioner.provision(...)` canonicalizes both expected and actual repository identities before comparing them.
- The macOS app still builds successfully on the current tree.

### Divergences

- A focused Proposal-007 non-UI verification path still cannot complete, because the scheme compiles the full `Chainworks ForgeTests` module and fails in `ProviderPlatformTests` on `AppConfiguration()` construction even when only Full-MVP tests are requested.
- `FastGate` no longer looks merely empty/mis-selected; it now reaches the test action but still reports `totalTestCount = 0` because the build fails first.
- Default product export locations currently contain no fresh delivery/sign-off packets for this round.

### Ambiguities / Evidence Gaps

- Local UI or app-launched proof was intentionally not attempted because the operator explicitly forbade UI runs on this laptop.
- The approved remote UI host policy exists, but the audit environment cannot authenticate to `SMacBook.local`, so remote app/UI proof could not be refreshed.
- Historical same-`HEAD` sample-repo exports from `R6` still exist under `/tmp/p007-r6-sample-happy` and `/tmp/p007-r6-sample-nonhappy`, but the current working tree contains substantial uncommitted Proposal-007-adjacent deltas, so those artifacts were treated as inherited context rather than authoritative current-round sign-off proof.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 8 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state executable plan
- Proposal Source: `5. Canonical live workflow for Proposal 007`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-run, inherited runtime
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `/tmp/p007-r8-build.xcresult`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/stage-summary.json`
- Gap / Note: The workflow file still ships on the buildable app path, and same-`HEAD` historical runtime evidence shows the sample-repo flow reached the expected repo-backed stages.

### REQ-002 Start Run, delivery preflight, run creation, and resume share one frozen `DeliveryConfiguration`
- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/RunRepository.swift`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
- Gap / Note: The shared freeze boundary remains visible in code on the current tree.

### REQ-003 One dedicated writable worktree is provisioned and persisted before the first implementation write
- Proposal Source: `7.1 Core rule`, `7.3 Persisted metadata`, `14. Acceptance criteria / Runtime / worktree`
- Status: Partially Implemented
- Evidence Type: code, inherited runtime
- Evidence:
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/run-metadata.json`
- Gap / Note: The provisioning/runtime path exists and inherited same-`HEAD` runtime evidence is encouraging, but current-round dogfood proof was not refreshed.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree
- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`, `14. Acceptance criteria / Runtime / worktree`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Proposal007DogfoodHarness.swift:189-205`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-95`
  - `Chainworks Forge/Engine/DeliveryConfiguration.swift:1-42`
- Gap / Note: The old basename-versus-Git-identity mismatch no longer appears in the current code path; both producer and consumer now canonicalize via the same normalizer.

### REQ-005 Approved implementation agents execute against the real provisioned worktree with explicit source context
- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`
- Status: Partially Implemented
- Evidence Type: code, tests-found, inherited runtime
- Evidence:
  - `Chainworks Forge/Engine/SourceContextBuilder.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/diff-summary.json`
- Gap / Note: The runtime path is present, but there is no fresh current-round repo-backed execution proof on the dirty tree.

### REQ-006 The implementation review/refine loop persists required artifacts and can iterate until `Implemented`
- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`
- Status: Partially Implemented
- Evidence Type: code, inherited runtime
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/docs-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/audit-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/security-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/prepush-review-report.json`
- Gap / Note: Required implementation-review artifacts exist in inherited same-`HEAD` evidence, but the audit did not re-prove a live refine-loop re-entry this round.

### REQ-007 Manual release remains explicit and a dedicated release-gate surface exists
- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `examples/workflows/full-mvp-live.yaml`
- Gap / Note: Manual release remains a first-class state and a dedicated surfaced view.

### REQ-008 Release side effects execute only through deterministic runtime services
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract`, `15. Locked decisions / ARCH-069`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Gap / Note: Deterministic service routing remains visible in the current runtime path.

### REQ-009 Commit/push emits `git_push_receipt` and `release_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, inherited runtime
- Evidence:
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/git-push-receipt.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/release-manifest.json`
- Gap / Note: The receipt/manifold path exists and has inherited same-`HEAD` sample proof, but current-round dogfood proof was not refreshed.

### REQ-010 Archive/distribute emits `connect_upload_receipt` and `release_bundle_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, inherited runtime
- Evidence:
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/connect-upload-receipt.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/release-bundle-manifest.json`
- Gap / Note: The archive/upload path exists and inherited same-`HEAD` sample proof exists, but there is no fresh current-round exported packet.

### REQ-011 Partial release failure preserves receipts and returns the run to blocked/operator recovery
- Proposal Source: `9.4 Partial failure semantics`, `14. Acceptance criteria / Release and Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, inherited runtime
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/delivery-receipt.json`
- Gap / Note: The blocked/recovery path is present and inherited non-happy-path proof exists, but it was not refreshed on the current dirty tree.

### REQ-012 The app exports a dogfood evidence pack with the promised delivery artifacts
- Proposal Source: `12.2 Evidence pack builder`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, inherited runtime
- Evidence:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
- Gap / Note: Export infrastructure and inherited same-`HEAD` packets exist, but the default current-round export locations are empty and no fresh packet was produced in this audit.

### REQ-013 Happy-path and non-happy-path repo-backed runs complete from inside the app with exported evidence
- Proposal Source: `12.3 Manual dogfood script`, `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`
- Status: Not Verifiable
- Evidence Type: runtime, inference
- Evidence:
  - `docs/reference/agent-ui-test-execution.md`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 test@SMacBook.local 'hostname && xcodebuild -version'`
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
  - `find "$HOME/Desktop" ...`
- Gap / Note: Same-`HEAD` historical sample-repo proof still exists under `/tmp`, but local UI execution was disallowed, remote auth failed, and the default product export locations contain no fresh current-round delivery/sign-off packets. That is not enough to re-close the proposal's dogfood sign-off on the present dirty tree.

### REQ-014 `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices
- Proposal Source: `14. Acceptance criteria / General`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - `/tmp/p007-r8-build.xcresult`
  - `/tmp/p007-r8-fast.xcresult`
  - `/tmp/p007-r8-unit.xcresult`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1380-1385`
  - `Chainworks Forge/Support/AppConfiguration.swift:17-30`
- Gap / Note: Build is green. Test is not. Both the direct `FastGate` run and the focused Full-MVP unit run fail at compile time with `Missing argument for parameter 'from' in call` in `ProviderPlatformTests.swift:1383` because `AppConfiguration` no longer exposes a zero-argument initializer. Both test actions therefore report `totalTestCount = 0`.

## Architecture Review

**Summary:** Acceptable

No fresh architecture finding is asserted in this round. The major `R7` architecture concern appears materially improved: the dogfood harness now freezes a canonical repository identifier, and the provisioner validates against the same normalization path instead of comparing raw basename and Git-derived forms.

## Product Review

**Summary:** At Risk

### PROD-001 Proposal-level dogfood closure is still not refreshed into product-default export paths
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-012`, `REQ-013`
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
  - `find "$HOME/Desktop" ...`
- Why It Matters: Same-`HEAD` historical sample-repo proof exists, which is a real improvement over a purely theoretical runtime path. But the current round still cannot show a fresh app-driven export in the product's default locations, so the sign-off story remains fragile and operator-dependent.
- Recommended Action: Re-run one happy-path and one non-happy-path delivery session on an approved remote Mac, then verify the exported evidence lands in the documented product-default storage or explicitly documented export destination.

## UI Review

**Summary:** Evidence Gap

No fresh UI/layout defect is asserted in this round. That is deliberate: the operator explicitly forbade local UI runs, and remote UI execution still could not authenticate from this environment. The missing proof here is a readiness problem, not a basis for inventing a UI defect.

## UX Review

**Summary:** At Risk

### UX-001 Proposal 007 sign-off still depends on evidence paths the operator cannot currently refresh from the documented workflow
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-013`, `REQ-014`
- Evidence Type: runtime, tests-run
- Evidence:
  - `docs/reference/agent-ui-test-execution.md`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
  - `/tmp/p007-r8-fast.xcresult`
  - `/tmp/p007-r8-unit.xcresult`
- Why It Matters: The repo now clearly documents remote-only UI execution, but the approved host path is not usable from this audit environment and the non-UI fallback is currently broken by compile regressions. That leaves the operator without a dependable sign-off route.
- Recommended Action: Restore one dependable proof path. Either provide working SSH auth to the approved remote Mac or fix the non-UI test gates so Proposal 007 can be re-proven without UI interaction.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Proposal-scoped non-UI verification is currently blocked by an unrelated provider-platform compile regression
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-014`
- Evidence Type: tests-run, code
- Evidence:
  - `/tmp/p007-r8-fast.xcresult`
  - `/tmp/p007-r8-unit.xcresult`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1380-1385`
  - `Chainworks Forge/Support/AppConfiguration.swift:17-30`
- Why It Matters: Both direct test actions failed before executing any tests because `ProviderPlatformTests` still constructs `AppConfiguration()` even though the type now requires explicit initialization parameters. That means Proposal 007's non-UI verification path is not isolated from unrelated test-target regressions and cannot currently close the proposal's general sign-off gate.
- Recommended Action: Either fix `ProviderPlatformTests` to use `AppConfiguration.seededDefault()` or an explicit fixture, or split a Proposal-007-specific verification path that does not compile unrelated provider-platform test code.

### READY-002 Remote-only UI proof is still unavailable from this environment
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-013`, `REQ-014`
- Evidence Type: runtime
- Evidence:
  - `docs/reference/agent-ui-test-execution.md`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 test@SMacBook.local 'hostname && xcodebuild -version'`
- Why It Matters: The repository's approved remote UI path is documented, but that does not help unless the current audit environment can authenticate to the remote host. With local UI runs prohibited, the absence of working remote auth leaves no current-round route to refresh Proposal 007's app-driven dogfood proof.
- Recommended Action: Provide a working SSH auth path to the approved remote Mac or document a noninteractive wrapper that handles remote invocation without manual operator setup.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Fresh build succeeded at `/tmp/p007-r8-build.xcresult`. |
| Core user flow runtime-validated | Partial | Same-`HEAD` sample-repo dogfood evidence still exists under `/tmp`, but it was not refreshed current-round. |
| Empty/loading/error states covered | Partial | Historical sample-repo happy/non-happy exports exist, but no fresh current-round UI proof was collected. |
| Accessibility risk acceptable | Not Checked | No fresh UI run was permitted locally, and remote UI execution did not authenticate. |
| Localization risk acceptable | Not Checked | No localization-specific validation ran in this audit. |
| Critical tests executed | Partial | Build ran; both test actions failed before any tests executed. |
| Privacy/permissions/entitlements reviewed | Partial | Remote-only host policy is explicit, but the environment lacks the credentials needed to use it. |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD && git rev-parse HEAD`
- `git status --short`
- `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `md5 -q docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
- `ssh -o BatchMode=yes -o ConnectTimeout=8 test@SMacBook.local 'hostname && xcodebuild -version'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p007-r8-build-dd' -resultBundlePath '/tmp/p007-r8-build.xcresult' build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p007-r8-fast-dd' -resultBundlePath '/tmp/p007-r8-fast.xcresult' -testPlan 'FastGate' test`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p007-r8-unit-dd' -resultBundlePath '/tmp/p007-r8-unit.xcresult' test -only-testing:'Chainworks ForgeTests/FullMVPWorkflowTests' -only-testing:'Chainworks ForgeTests/FullMVPReleaseTests' -only-testing:'Chainworks ForgeTests/FullMVPIntegrationTests' -only-testing:'Chainworks ForgeTests/WorktreeProvisionerTests'`
- `xcrun xcresulttool get build-results summary --path '/tmp/p007-r8-build.xcresult'`
- `xcrun xcresulttool get build-results summary --path '/tmp/p007-r8-fast.xcresult'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p007-r8-fast.xcresult'`
- `xcrun xcresulttool get build-results summary --path '/tmp/p007-r8-unit.xcresult'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p007-r8-unit.xcresult'`
- `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 \( -name 'delivery_receipt*.json' -o -name 'evidence-pack*.zip' -o -name 'evidence_manifest*.json' -o -name 'signoff_evidence_manifest*.json' \) -print | sort`
- `find "$HOME/Desktop" -maxdepth 2 \( -name '*evidence*pack*' -o -name '*delivery*receipt*' -o -name '*signoff*' \) -print | sort`
- `stat -f '%Sm %N' -t '%Y-%m-%d %H:%M:%S %z' /tmp/p007-r6-sample-happy /tmp/p007-r6-sample-nonhappy`

## Recommended Next Actions

1. Fix the compile regression in `Chainworks ForgeTests/ProviderPlatformTests.swift` so both `FastGate` and focused non-UI Proposal 007 test paths can execute real tests again.
2. Re-run one happy-path and one non-happy-path repo-backed delivery session on an approved remote Mac once SSH auth is available.
3. Verify that the refreshed exported evidence lands in the documented product-default locations or tighten the documentation so the expected export destination is unambiguous.
4. After the fresh remote dogfood rerun, re-audit `REQ-013` and `REQ-014` together rather than treating historical `/tmp` artifacts as the primary sign-off proof.
