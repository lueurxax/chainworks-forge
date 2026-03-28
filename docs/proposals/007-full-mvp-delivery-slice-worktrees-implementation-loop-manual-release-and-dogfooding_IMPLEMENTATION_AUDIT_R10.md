# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R10

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T17:46:44+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Implemented` |
| Overall Readiness | `Ready with Risks` |
| Audit Confidence | `Medium` |

## Executive Verdict

Proposal 007 is now `Implemented` on the current `HEAD`. The previously accepted same-`HEAD` app-launched happy-path and non-happy-path dogfood artifacts remain valid proposal-level proof, and the two remaining gaps from `R9` are now closed by approved-host remote evidence: the canonical gate path (`./scripts/test-gate.sh build` and `./scripts/test-gate.sh fast`) is green on `SMacBook.local`, and the repo-backed implementation review/refine loop now has fresh direct runtime proof through a green integration bundle that includes `Repo-backed fixture implementation review refine loop re-enters review and completes`. The remaining caution is evidentiary, not contractual: those new remote results were operator-confirmed rather than independently replayed from this environment.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | No in-scope requirement remains open | Medium |
| Architecture | Acceptable | No fresh architecture blocker surfaced | High |
| Product | Strong | Repo-backed happy-path and non-happy-path dogfood proof is already in hand | High |
| UI | Acceptable | No new UI contract gap surfaced for Proposal 007 | Medium |
| UX | Acceptable | The operator journey is evidenced by real app-launched dogfood artifacts | Medium |
| Readiness | Ready with Risks | Latest remote sign-off proof was operator-confirmed, not independently replayed here | Medium |

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
- The repo identity contract remains coherent across the dogfood harness and the provisioner.
- The refine-loop contract now has a dedicated repo-backed fixture test.
- The repository-documented canonical gate path is green on the approved remote host per operator-confirmed remote execution.

### Divergences

- No live proposal-conformance divergence remains open.

### Ambiguities / Evidence Gaps

- The latest remote green gate results and integration bundle were operator-confirmed on `SMacBook.local`, not independently replayed from this audit environment.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 14 |
| Partially Implemented | 0 |
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
- Status: Implemented
- Evidence Type: tests-run, runtime
- Evidence:
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:1081-1108`
  - operator-confirmed remote result bundle `/tmp/p007-r10-integration-2.xcresult`
  - operator-confirmed remote summary: `11 tests`, `0 failures`
- Gap / Note: The previous `R9` gap is now closed by direct repo-backed integration proof that the implementation review/refine loop re-enters review and still completes.

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
- Gap / Note: These are same-`HEAD` app-launched dogfood artifacts with real run-storage paths and exported evidence packs, which matches the documented proposal-level proof contract.

### REQ-014 `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices
- Proposal Source: `14. Acceptance criteria / General`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `scripts/test-gate.sh:395-417`
  - operator-confirmed remote gate run on `SMacBook.local`: `./scripts/test-gate.sh build`
  - operator-confirmed remote gate run on `SMacBook.local`: `./scripts/test-gate.sh fast`
  - operator-confirmed remote bundle `/tmp/p007-r10-integration-2.xcresult`
- Gap / Note: The canonical gate path is now green on the approved remote host. The old local diagnostic compile regression is no longer the sign-off truth for this proposal.

## Architecture Review

**Summary:** Acceptable

No fresh architecture finding surfaced in this round. The previous repo-identity concern remains closed.

## Product Review

**Summary:** Strong

No fresh product blocker surfaced in this round. Proposal 007 now has both app-launched dogfood proof and direct suite-level integration proof for the refine loop.

## UI Review

**Summary:** Acceptable

No fresh Proposal-007-specific UI finding surfaced in this round. The proposal's canonical proof is repo-backed dogfood plus approved-host validation, and that contract is satisfied.

## UX Review

**Summary:** Acceptable

No fresh UX blocker surfaced in this round. Happy-path and blocked-path operator journeys are already evidenced by live dogfood artifacts.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Latest green proof is operator-confirmed remote evidence rather than locally replayed evidence
- Severity: Note
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-006`, `REQ-014`
- Evidence Type: tests-run
- Evidence:
  - operator-confirmed remote host `SMacBook.local`
  - operator-confirmed remote gate results: `./scripts/test-gate.sh build`, `./scripts/test-gate.sh fast`
  - operator-confirmed remote result bundle `/tmp/p007-r10-integration-2.xcresult`
- Why It Matters: The proposal contract is now closed, but the final green gate evidence came from the approved remote host and was reported into this audit rather than independently replayed from the current environment.
- Recommended Action: Keep the remote bundle and gate output archived with the rest of the Proposal 007 sign-off evidence so the proof chain stays durable.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Operator-confirmed remote `./scripts/test-gate.sh build` green on approved host. |
| Core user flow runtime-validated | Pass | Same-`HEAD` app-launched happy-path and non-happy-path repo-backed proof exists with real run-storage and exported evidence packs. |
| Empty/loading/error states covered | Pass | Blocked delivery is proven by the non-happy-path dogfood packet. |
| Accessibility risk acceptable | Not Checked | No fresh accessibility-specific validation ran in this audit. |
| Localization risk acceptable | Not Checked | No localization-specific validation ran in this audit. |
| Critical tests executed | Pass | Operator-confirmed remote `./scripts/test-gate.sh fast` green; remote integration bundle reports `11` tests, `0` failures. |
| Privacy/permissions/entitlements reviewed | Partial | Remote-only UI policy is explicit and approved-host path is being used, but this audit did not independently inspect entitlements beyond existing flows. |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD && git rev-parse HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `md5 -q docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `sed -n '1,260p' docs/reference/agent-ui-test-execution.md`
- `sed -n '1,260p' scripts/test-gate.sh`
- `rg -n "Repo-backed fixture implementation review refine loop re-enters review and completes|refine loop re-enters review" 'Chainworks ForgeTests'`
- `./scripts/test-gate.sh build` on `SMacBook.local` -> operator-confirmed green
- `./scripts/test-gate.sh fast` on `SMacBook.local` -> operator-confirmed green
- suite-level repo-backed integration proof on `SMacBook.local` -> operator-confirmed `/tmp/p007-r10-integration-2.xcresult`, `11 tests`, `0 failures`
- `sed -n '1,220p' /tmp/p007-r6-sample-happy/result.json`
- `sed -n '1,220p' /tmp/p007-r6-sample-nonhappy/result.json`
- `find /tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974 -maxdepth 2 -type f | sort`
- `find /tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9 -maxdepth 2 -type f | sort`

## Recommended Next Actions

1. Archive the approved-host gate outputs and `/tmp/p007-r10-integration-2.xcresult` alongside the existing Proposal 007 evidence so the sign-off chain stays durable.
2. If Proposal 008 depends on 007 being fully green, use this `R10` audit as the new prerequisite baseline instead of the older `R8`/`R9` reports.
