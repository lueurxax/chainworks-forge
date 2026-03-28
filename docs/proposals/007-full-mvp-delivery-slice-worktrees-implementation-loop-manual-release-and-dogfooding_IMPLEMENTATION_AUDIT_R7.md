# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R7

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T15:41:04+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 007 remains `Partial` on the current tree. The repo-backed delivery architecture is materially present: the shared run-creation freeze still exists, deterministic release services are wired, the dogfood harness is in-tree, and the app builds cleanly on macOS. The proposal still does not reach `Implemented`, because this round could not refresh the proposal-required happy-path and non-happy-path app-launched proof without violating the operator's local-UI prohibition, the approved remote host path is documented but not reachable from this environment due missing SSH auth, the self-repo dogfood identity contract still looks brittle in code, and the current test verification surface is partially regressed: a unit-focused `xcodebuild test` path fails while compiling `Chainworks ForgeUITests`, and the repository-supported `FastGate` plan currently executes `0` tests.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Proposal-level dogfood proof was not refreshed on current `HEAD` | High |
| Architecture | Acceptable | Self-repo repository identity is still frozen in a form the provisioner may reject | High |
| Product | At Risk | The first believable repo-backed dogfood path is not currently re-proven on this tree | High |
| UI | Evidence Gap | Local UI execution was forbidden and remote UI execution was not reachable from this environment | High |
| UX | At Risk | A self-dogfood operator can still hit a low-level repo-identity failure before implementation starts | Medium |
| Readiness | Not Ready | Current verification gates are not strong enough to close Proposal 007 sign-off | High |

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
- Screenshot proof for release-gate/final-receipt review.
- Green `xcodebuild build && xcodebuild test`.

### Explicit Exclusions

- Proposal 007 does not replace the operator-shell or provider-platform baselines.
- Proposal 007 does not introduce production release targets.
- Proposal 007 does not require a full repo browser or multi-repo orchestration surface.

## Proposal Fidelity / Divergence

### Matches

- `RunStartSnapshot` still freezes delivery truth and applies it atomically to `Run` at creation time.
- `RunRepository.createRunFromPlan(...)` remains the shared run-creation boundary for that frozen start snapshot.
- `DeliveryReceiptBuilder` is still wired from the real runtime path in `WorkflowOrchestrator`.
- The repository now explicitly documents and enforces remote-only UI policy instead of relying on operator lore.
- The macOS app builds successfully on the current tree.

### Divergences

- The self-repo dogfood path still freezes `repoIdentifier` as `lastPathComponent`, while worktree provisioning validates against Git-derived repository identity.
- A unit-focused `xcodebuild test` path still fails while compiling `Chainworks ForgeUITests` because `setUpWithError()` is overridden twice in the same file.
- The repository-supported `FastGate` test plan currently returns `0` executed tests, so it is green but non-proving.

### Ambiguities / Evidence Gaps

- Local UI or app-launched proof was intentionally not attempted because the operator explicitly forbade UI runs on this laptop.
- The approved remote UI host policy exists, but the audit environment has no SSH identities and cannot authenticate to `SMacBook.local`, so remote app/UI proof could not be refreshed.
- Historical `R6` sample-repo happy-path and non-happy-path exports exist, but this tree has substantial uncommitted Proposal 007 deltas, so those old artifacts were not treated as authoritative proof for `R7`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 9 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state executable plan
- Proposal Source: `5. Canonical live workflow for Proposal 007`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `/tmp/p007-r7-build.xcresult`
  - `Chainworks Forge.app` build copied `full-mvp-live.yaml` into app resources during the fresh build
- Gap / Note: The current tree still bundles the repo-backed preset into the app, and the app build succeeded.

### REQ-002 Start Run, delivery preflight, run creation, and resume share one frozen `DeliveryConfiguration`
- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:3-38`
  - `Chainworks Forge/Models/RunRepository.swift:98-136`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:80-96`
- Gap / Note: The shared freeze boundary is still present in code on current `HEAD`, even though no fresh current-round dogfood runtime was executed.

### REQ-003 One dedicated writable worktree is provisioned and persisted before the first implementation write
- Proposal Source: `7.1 Core rule`, `7.3 Persisted metadata`, `14. Acceptance criteria / Runtime / worktree`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
- Gap / Note: The provisioning/runtime path exists, but current-round runtime proof was not refreshed because local app/UI execution was forbidden and remote proof was unavailable.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree
- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`, `14. Acceptance criteria / Runtime / worktree`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Proposal007DogfoodHarness.swift:189-214`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-88`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift:80-98`
- Gap / Note: Guard code exists, but the self-repo path still appears brittle because the harness freezes `repoIdentifier` as the repo folder basename while the provisioner validates against Git-derived identity.

### REQ-005 Approved implementation agents execute against the real provisioned worktree with explicit source context
- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/SourceContextBuilder.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Gap / Note: The runtime path is present, but no fresh current-round repo-backed execution proof was collected.

### REQ-006 The implementation review/refine loop persists required artifacts and can iterate until `Implemented`
- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The loop and artifact surfaces exist, but no current-round lived implementation-review/refine loop was executed.

### REQ-007 Manual release remains explicit and a dedicated release-gate surface exists
- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `examples/workflows/full-mvp-live.yaml`
- Gap / Note: Manual release remains a first-class state and has a dedicated surfaced view.

### REQ-008 Release side effects execute only through deterministic runtime services
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract`, `15. Locked decisions / ARCH-069`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Gap / Note: The deterministic service routing is visible in code on current `HEAD`.

### REQ-009 Commit/push emits `git_push_receipt` and `release_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift`
- Gap / Note: Receipt/manifold generation logic exists, but fresh current-round repo-backed runtime receipts were not collected.

### REQ-010 Archive/distribute emits `connect_upload_receipt` and `release_bundle_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift`
- Gap / Note: The archive/upload path exists in code, but no fresh exported happy-path packet was collected on the current tree.

### REQ-011 Partial release failure preserves receipts and returns the run to blocked/operator recovery
- Proposal Source: `9.4 Partial failure semantics`, `14. Acceptance criteria / Release and Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Gap / Note: The blocked/recovery model exists, but the proposal-specific non-happy-path runtime was not refreshed in this round.

### REQ-012 The app exports a dogfood evidence pack with the promised delivery artifacts
- Proposal Source: `12.2 Evidence pack builder`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
- Gap / Note: Export infrastructure is present and wired, but no fresh current-round exported evidence pack was produced.

### REQ-013 Happy-path and non-happy-path repo-backed runs complete from inside the app with exported evidence
- Proposal Source: `12.3 Manual dogfood script`, `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`
- Status: Not Verifiable
- Evidence Type: runtime, inference
- Evidence:
  - `docs/reference/agent-ui-test-execution.md:127-142`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && pwd'`
  - `ssh-add -l`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R6.md`
- Gap / Note: Local app/UI execution was explicitly disallowed by the operator, and the approved remote host path could not be authenticated from this environment (`Permission denied`, no SSH identities loaded). Older `R6` sample-repo exports exist, but Proposal 007 files changed materially afterward, so they were treated as historical context only.

### REQ-014 `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices
- Proposal Source: `14. Acceptance criteria / General`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - `/tmp/p007-r7-build.xcresult`
  - `/tmp/p007-r7-unit.xcresult`
  - `/tmp/p007-r7-fastplan.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:13-16`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:387-389`
  - `TestPlans/FastGate.xctestplan`
- Gap / Note: Build is green. The current test story is not: a unit-focused `xcodebuild test` path fails while compiling `Chainworks ForgeUITests` because `setUpWithError()` is declared twice, and the repo-supported `FastGate` plan reports `0` executed tests despite non-empty `selectedTests`.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 Self-repo repository identity is still brittle in the dogfood harness path
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`, `REQ-004`, `REQ-013`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/Proposal007DogfoodHarness.swift:193-206`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-88`
- Why It Matters: The current harness still freezes `repoIdentifier` as the workspace folder name, while provisioning validates against Git-derived identity. That is the same shape of mismatch that previously blocked the self-repo dogfood flow before implementation started.
- Recommended Action: Canonicalize repository identity at the shared `DeliveryConfiguration` creation boundary so basename, slug, and remote-origin forms resolve to one stable runtime value.

## Product Review

**Summary:** At Risk

### PROD-001 Proposal-level dogfood closure is blocked by verification infrastructure, not just by missing feature code
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-013`, `REQ-014`
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p007-r7-build.xcresult`
  - `/tmp/p007-r7-unit.xcresult`
  - `/tmp/p007-r7-fastplan.xcresult`
  - `docs/reference/agent-ui-test-execution.md:111-142`
- Why It Matters: The product slice is close enough that sign-off now depends on trustworthy proof. Right now that proof path is broken from both sides: local UI runs are forbidden by operator instruction, remote UI proof is documented but unreachable from this environment, and the non-UI gate is partially non-proving.
- Recommended Action: Restore one trustworthy proof path. Either provide remote-host auth for the approved Mac or introduce a repo-supported non-UI Proposal 007 verification path that does not compile `Chainworks ForgeUITests`.

## UI Review

**Summary:** Evidence Gap

No fresh UI/layout finding was asserted in this round. That is deliberate: the operator explicitly forbade local UI runs, and remote UI execution could not be authenticated from this environment. The missing proof here is a readiness problem, not a basis for inventing a UI defect.

## UX Review

**Summary:** At Risk

### UX-001 The self-dogfood path still appears likely to fail with a low-level provisioning truth mismatch
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-004`, `REQ-013`
- Evidence Type: code, inference
- Evidence:
  - `Chainworks Forge/Engine/Proposal007DogfoodHarness.swift:193-206`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-88`
- Why It Matters: Even if the sample-repo path remains viable, the current self-repo path still appears to freeze a different identity form than the provisioner accepts. That is the kind of failure an engineer can debug, but not the low-drama dogfood journey the proposal promises.
- Recommended Action: Add one canonical repo-identity derivation path and surface a preflight-level operator message before worktree provisioning if the target repo identity is ambiguous.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The current unit verification path is partially broken
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-014`
- Evidence Type: tests-run, code
- Evidence:
  - `/tmp/p007-r7-unit.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:13-16`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:387-389`
- Why It Matters: A unit-focused `xcodebuild test` attempt currently fails before tests run because the scheme still builds `Chainworks ForgeUITests`, and that target now contains two `setUpWithError()` overrides. That blocks one obvious non-UI verification path.
- Recommended Action: Remove the duplicate UITest override or split a test-only unit scheme/test plan that never compiles the UI target for non-UI gates.

### READY-002 The repository-supported `FastGate` is green but non-proving
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-014`
- Evidence Type: tests-run, code
- Evidence:
  - `/tmp/p007-r7-fastplan.xcresult`
  - `TestPlans/FastGate.xctestplan`
- Why It Matters: The plan contains non-empty `selectedTests`, but the current run reported `totalTestCount = 0`, `passedTests = 0`, `result = unknown`. A green empty gate cannot close proposal sign-off.
- Recommended Action: Fix the `FastGate` selection semantics until the plan executes the intended unit/runtime suites and returns a non-zero test count.

### READY-003 Remote-only UI proof is documented but unavailable from this audit environment
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-013`, `REQ-014`
- Evidence Type: runtime
- Evidence:
  - `docs/reference/agent-ui-test-execution.md:111-142`
  - `scripts/test-gate.sh:34-88`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && pwd'`
  - `ssh-add -l`
- Why It Matters: The repository now clearly defines approved remote UI hosts, but that does not help unless the audit environment can actually authenticate to one. Without that, the operator's no-local-UI rule leaves no current-round path to refresh Proposal 007's dogfood evidence.
- Recommended Action: Provide a working SSH auth path to an approved remote host or document a repo-supported remote execution wrapper that does not depend on ad hoc operator setup.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Fresh build succeeded at `/tmp/p007-r7-build.xcresult`. |
| Core user flow runtime-validated | Partial | No fresh current-round happy/non-happy app-launched proof was collected because local UI runs were forbidden and remote auth was unavailable. |
| Empty/loading/error states covered | Partial | Historical Proposal 007 evidence exists, but it was not treated as current proof because Proposal 007 implementation files changed materially. |
| Accessibility risk acceptable | Not Checked | No fresh UI run was permitted locally, and remote UI execution did not authenticate. |
| Localization risk acceptable | Not Checked | No localization-specific validation ran in this audit. |
| Critical tests executed | Partial | Build ran; unit-focused test path failed at UITest-target compile; `FastGate` ran but executed `0` tests. |
| Privacy/permissions/entitlements reviewed | Partial | Remote-only host policy is explicit, but the environment lacks the credentials needed to use it. |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short=7 HEAD && git rev-parse HEAD`
- `git status --short -- 'Chainworks Forge/**' 'Chainworks ForgeTests/**' 'Chainworks ForgeUITests/**' 'examples/workflows/**' 'docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md'`
- `git diff --name-only -- 'Chainworks Forge/**' 'Chainworks ForgeTests/**' 'Chainworks ForgeUITests/**' 'examples/workflows/**' 'docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md'`
- `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `md5 -q docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `sed -n '1,260p' docs/reference/agent-ui-test-execution.md`
- `sed -n '1,260p' scripts/test-gate.sh`
- `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && pwd'`
- `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook 'hostname && pwd'`
- `sed -n '1,220p' ~/.ssh/config`
- `sed -n '1,220p' ~/.orbstack/ssh/config`
- `ssh-add -l`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p007-r7-build-dd' build -resultBundlePath '/tmp/p007-r7-build.xcresult'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p007-r7-unit-dd' -only-testing:'Chainworks ForgeTests/FullMVPWorkflowTests' -only-testing:'Chainworks ForgeTests/FullMVPReleaseTests' -only-testing:'Chainworks ForgeTests/FullMVPIntegrationTests' -only-testing:'Chainworks ForgeTests/WorktreeProvisionerTests' test -resultBundlePath '/tmp/p007-r7-unit.xcresult'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -testPlan 'FastGate' test -resultBundlePath '/tmp/p007-r7-fastplan.xcresult'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p007-r7-unit.xcresult'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p007-r7-fastplan.xcresult'`
- `xcodebuild -list -project 'Chainworks Forge.xcodeproj'`

## Recommended Next Actions

1. Fix the duplicate `setUpWithError()` override in `Chainworks ForgeUITests` or split a unit-only verification path so non-UI Proposal 007 tests can run without compiling the UI target.
2. Repair `FastGate` so it executes its selected tests instead of returning a green `0`-test result.
3. Canonicalize repo identity at the shared `DeliveryConfiguration` boundary so self-repo dogfood no longer depends on basename-vs-origin string matching.
4. Re-run one happy-path and one non-happy-path app-launched Proposal 007 proof on an approved remote host once SSH auth is available, then collect the fresh exported evidence packs on that host.
