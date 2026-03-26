# Proposal 007: Full MVP Delivery Slice — Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md |
| Repository Root | . |
| Git SHA | 0b2ca31 |
| Working Tree | dirty |
| Audited At | 2026-03-26T07:46:16+0200 |
| Proposal State | Active (Draft) |
| Overall Status | Not Implemented |

## Verdict

Proposal 007 is not fully implemented on the current tree. The repo already contains a real `full-mvp-live.yaml` preset, a delivery-configuration UI slice, worktree provisioning primitives, a release-gate surface, and an evidence-pack exporter. But the core repo-backed contract is still open: implementation agents do not actually execute against the provisioned worktree, release side effects are not wired through deterministic runtime services, `ConnectPublishService` is still scaffold-only, and there is no authoritative happy-path plus non-happy-path dogfood proof from inside the app. Because several in-scope runtime requirements remain missing, the overall audit verdict is `Not Implemented`.

## Proposal Contract

### Scope

- Deliver the first repo-backed 12-state end-to-end workflow from idea through completed release candidate inside the app.
- Add one dedicated writable worktree per run for implementation and release-related execution.
- Run real implementation/review/release agents against a repository-backed target.
- Keep manual release gating explicit and route release side effects through deterministic services.
- Ship a dogfood-ready preset, sample repo profile, and exportable evidence pack.
- Extend provider-platform surfaces with delivery preflight, repo/release selection, and dogfood-oriented onboarding.

### Locked Decisions

- One run equals one dedicated writable worktree.
- No concurrent write-capable agents may share a writable worktree.
- Release mechanics run through deterministic services, not free-form agent shelling.
- `full-mvp-live.yaml` is separate from the fast `proposal-loop-live.yaml` smoke path.
- `docs_report` must exist before audit aggregation in the first implementation review cycle.
- Default release targets are `sandbox` and `staging`, not production.
- Partial release failure returns to blocked/operator recovery rather than hidden rollback.
- Approval gates remain explicit workflow states.

### Acceptance Criteria

- A dedicated writable worktree is provisioned and persisted before the first implementation write.
- No write-capable action can escape `worktreeRoot` / `workspaceRoot`, and concurrent runs cannot share a writable worktree.
- `full-mvp-live.yaml` compiles into a valid 12-state executable plan with explicit approval states.
- The implementation review/refine loop produces the required artifacts and can iterate until `Implemented`.
- Manual release blocks on explicit human approval.
- Release side effects execute only through deterministic services and produce durable manifests/receipts.
- Start Run, preflight, run creation, and resume share the same frozen `DeliveryConfiguration`.
- Run Progress, Release Gate, report/recovery/comparison, and provider-baseline surfaces work for repo-backed runs.
- A happy-path and non-happy-path dogfood run can be completed from inside the app with exported evidence.
- `xcodebuild build && xcodebuild test` is green with no regressions in earlier proposal slices.

### Test / Evidence Requirements

- Unit tests for worktree/repo safety, workflow structure, implementation loop, and release ops.
- Safe local integration tests for full sample-repo runs, blocked-release recovery, resume, and release rejection.
- Env-gated live smoke tests for sandbox push/upload and a full dogfood run.
- Evidence-based review proof: one happy-path run, one non-happy-path run, exported evidence pack, and screenshots for release gate plus final receipts.

### Explicit Exclusions

- Multiple concurrent write-capable agents in the same worktree.
- Autonomous release with no human gate.
- Automatic rollback after push/upload.
- Multi-repo orchestration.
- Background/cloud execution or Temporal migration.
- Automatic workflow mutation via Steward.
- Production-by-default release targets.
- Multi-user/team coordination surfaces.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 1 |
| Partially Implemented | 8 |
| Missing | 4 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state plan with explicit approval gates

- Proposal Source: `2. Product question this proposal must answer` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:96-120`), `11.1 Parse approval_policy` (`...:827-848`), `11.2 Add full-mvp-live.yaml` (`...:850-857`), `14. Acceptance criteria / Workflow` (`...:1003-1010`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:1-370`
  - `Chainworks Forge/Engine/RunPlan.swift:57-68`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift:294-345`
  - `/tmp/p007-compiler.xcresult` — `RunPlanCompilerTests/fullMVPLiveWorkflowCompiles()` passed
- Gap / Note: This closes the compile-time workflow contract only. It does not by itself prove the repo-backed runtime path.

### REQ-002 Start Run, delivery preflight, and run creation share one frozen `DeliveryConfiguration`

- Proposal Source: `6.4 Delivery configuration is a first-class boundary` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:323-356`), `6.5 Sample repo profile schema stays subordinate` (`...:358-376`), `9.6 Delivery preflight extends the provider-platform baseline` (`...:716-733`), `10.1 Dogfood Start Run preset` (`...:741-770`), `14. Acceptance criteria / UI` (`...:1018-1024`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:793-916`
  - `Chainworks Forge/Views/IdeaListView.swift:1148-1203`
  - `Chainworks Forge/Engine/DeliveryConfiguration.swift:5-71`
  - `Chainworks Forge/Engine/DeliveryPreflightService.swift:26-100`
  - `/tmp/p007-audit.xcresult` — `DeliveryServicesTests/deliveryConfigCodable()`, `preflightMissingRepo()`, and `preflightEmptyReleaseTarget()` passed
- Gap / Note: The editable draft and persisted `deliveryConfigurationJSON` exist, but the freeze happens after `compiler.createRun(...)`, not inside the `createRun()`/repository boundary the proposal defines. The mirrored scalar fields on `Run` (`repoIdentifier`, `repoRoot`, `baseBranch`, `targetBranch`, `releaseTargetID`, `releaseMode`) are not populated at start, and the current UI hardcodes a dogfood profile shape instead of a complete profile-or-direct-target contract.

### REQ-003 The orchestrator provisions and persists one dedicated writable worktree before implementation begins

- Proposal Source: `7.1 Core rule` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:382-391`), `7.2 Worktree identity` (`...:393-407`), `7.3 Persisted metadata` (`...:409-439`), `7.4 Provisioning rules` (`...:441-450`), `8.1 Handoff from approved proposal` (`...:485-500`), `14. Acceptance criteria / Runtime / worktree` (`...:997-1002`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:309-331`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:720-745`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:50-129`
  - `/tmp/p007-audit.xcresult` — `WorktreeProvisionerTests/createsUniqueWorktreePerRun()`, `provisioningResultContainsBaseRevision()`, and related provisioning tests passed
- Gap / Note: The orchestrator does provision before the state run block and persists `run.worktreeRoot` plus `run.baseRevision`, but it does not populate the other mirrored run metadata promised in `§7.3`, and this audit did not close the resume/runtime proof for an actual repo-backed run crossing into implementation.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree

- Proposal Source: `7.5 No shared write worktrees` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:452-457`), `7.7 Path boundary enforcement` (`...:472-479`), `14. Acceptance criteria / Runtime / worktree` (`...:997-1002`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift:5-84`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:102-123`
  - `/tmp/p007-audit.xcresult` — `WorktreeProvisionerTests/noConcurrentWritableAgentUsesSharedWorktree()`, `DeliveryServicesTests/safetyGuardRejectsTraversal()`, `safetyGuardRejectsOutside()`, and `safetyGuardRepoMismatch()` passed
- Gap / Note: The guard APIs and focused tests exist, but the runtime does not yet enforce them before every file operation or tool call in the repo-backed implementation path. Because write-capable agent execution is not actually switched onto the provisioned worktree, this safety boundary is only partially integrated.

### REQ-005 Approved implementation agents execute against the real provisioned worktree and explicit source context

- Proposal Source: `4. Scope / In scope items 2-3` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:171-187`), `3. What we build / Layer I` (`...:128-139`), `8. Implementation slice` (`...:483-589`), `14. Acceptance criteria / Workflow` (`...:1003-1009`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:458-468`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:43-55`
  - `Chainworks Forge/Engine/SourceContextBuilder.swift:5-53`
  - `rg -n "SourceContextBuilder|ImplementationDeliveryPreset" 'Chainworks Forge' 'Chainworks ForgeTests'` — runtime hits only in the type file itself and test fixtures
- Gap / Note: `ExecutionContext` still carries the original `workspace`, `GooseSessionBridge` still uses `context.workspace.workspaceRoot.path` as the working directory, and `SourceContextBuilder` is not wired into execution. The provisioned `run.worktreeRoot` never becomes the authoritative execution cwd for `code_writer` or the review quartet. This is the core missing repo-backed contract.

### REQ-006 The implementation review/refine loop persists the required artifacts and can iterate until `Implemented`

- Proposal Source: `8.2 Continue until seemingly complete` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:510-526`), `8.3 Implementation reviewed against proposal` (`...:527-560`), `8.4 Implementation refined` (`...:562-589`), `14. Acceptance criteria / Workflow` (`...:1003-1009`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:202-317`
  - `Chainworks Forge/Engine/OutputContractTemplates.swift:22-32`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:92-163`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:120-143`
- Gap / Note: The workflow structure and artifact names are in place, but the strongest test coverage for the state-8/state-9/state-10 loop currently lives in `FullMVPDeliveryTests.swift`, and that Swift Testing suite was not actually executed by the focused `xcodebuild` selector used in this audit. More importantly, because implementation agents are not really operating in the provisioned worktree, the “review against live code changes” requirement is only partially satisfied.

### REQ-007 Manual release stays explicit and the operator gets a dedicated release-gate surface with sufficient context

- Proposal Source: `9.1 Release must remain explicit` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:595-612`), `10.3 Release Gate View` (`...:787-809`), `11.1 Parse approval_policy` (`...:827-848`), `14. Acceptance criteria / Workflow and UI` (`...:1009-1024`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:319-347`
  - `Chainworks Forge/Engine/RunPlan.swift:65-68`
  - `Chainworks Forge/Views/IdeaListView.swift:1573-1597`
  - `Chainworks Forge/Views/ReleaseGateView.swift:6-221`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:165-240`
- Gap / Note: The explicit `manual_release` approval state and tailored gate view exist, but the gate still falls short of the promised operator context. It does not surface the quick actions (`open proposal`, `open diff summary`, `open docs delta`, `open receipts/report`), and the above-the-fold summary still lacks the concrete diff-stat, changed-file, and spend framing promised by `§10.3`.

### REQ-008 Release side effects execute only through deterministic runtime services rather than simulated artifact generation

- Proposal Source: `3. What we build / Layer I` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:128-139`), `9.2 Release step sequence` (`...:613-631`), `9.3 Service contract` (`...:632-678`), `14. Acceptance criteria / Release` (`...:1011-1016`), `15. Locked decisions / ARCH-069` (`...:1047-1055`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift:44-108`
  - `Chainworks Forge/Engine/OutputContractTemplates.swift:16-36`
  - `Chainworks Forge/Engine/OutputContractTemplates.swift:199-214`
  - `rg -n "ReleaseOpsCoordinator|executeRelease\\(" 'Chainworks Forge' 'Chainworks ForgeTests'` — no production call site; only the type file and tests
- Gap / Note: The deterministic services exist as standalone types, but the runtime does not invoke them as the sole release path. Release contracts can still be satisfied by generic simulated artifact generation (`git_push_receipt_v1`, `connect_upload_receipt_v1`) rather than coordinator-driven side effects. That is a direct miss against ARCH-069 and the acceptance criteria.

### REQ-009 Commit/push produces a real `release_manifest` and `git_push_receipt` for the repo-backed workflow

- Proposal Source: `9.2 Release step sequence` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:615-631`), `9.3 Service contract / GitReleaseService` (`...:634-657`), `14. Acceptance criteria / Release` (`...:1011-1016`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/GitReleaseService.swift:56-142`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:243-286`
- Gap / Note: `GitReleaseService` has a credible commit/push implementation and structured receipt types, but it is not wired into the production state-11 path. This audit also has no current-tree runtime proof of a repo-backed run producing the manifest/receipt pair from inside the app.

### REQ-010 Archive/distribute produces a real `release_bundle_manifest` and `connect_upload_receipt`

- Proposal Source: `9.2 Release step sequence` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:623-631`), `9.3 Service contract / ConnectPublishService` (`...:658-678`), `14. Acceptance criteria / Release` (`...:1011-1016`)
- Status: Missing
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ConnectPublishService.swift:59-99`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:288-326`
- Gap / Note: `ConnectPublishService` is explicitly still a scaffold for the first dogfood slice. It returns `archivePath = nil`, `sizeBytes = 0`, and uses the commit SHA as a proxy checksum. It is also not wired into the runtime release stage. That does not meet the proposal’s “real archive/upload” contract.

### REQ-011 Partial release failure preserves receipts and returns the run to an operator-visible blocked recovery path

- Proposal Source: `9.4 Partial failure semantics` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:680-701`), `14. Acceptance criteria / Release and Dogfooding` (`...:1011-1030`), `15. Locked decisions / ARCH-073` (`...:1053-1053`)
- Status: Missing
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift:48-107`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:328-377`
- Gap / Note: The partial-failure shape exists only inside `ReleaseOpsCoordinator.ReleaseResult` and a synthetic test fixture. There is no orchestrator/runtime path persisting the git-side receipts, marking the run blocked, and re-entering a release-context recovery UI after an actual release failure.

### REQ-012 Evidence Pack Builder exports the complete dogfood packet promised by the proposal

- Proposal Source: `12.2 Evidence pack builder` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:898-925`), `13.3 Evidence-based review requirement` (`...:983-991`), `14. Acceptance criteria / Dogfooding` (`...:1026-1030`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift:18-129`
  - `Chainworks Forge/Views/RunsHomeView.swift:560-595`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:584-637`
- Gap / Note: The exporter exists and writes a usable skeleton pack, but it does not curate the full promised packet as explicit top-level deliverables (`run report`, `proposal draft`, `implementation review summary`, `docs delta`, `support bundle`, release receipts). It mainly copies raw artifacts plus metadata and a checklist, and because core delivery mirror fields are not populated reliably, exported metadata can still degrade to `"none"` for important repo/release fields.

### REQ-013 Existing operator/report/recovery/provider-baseline surfaces apply cleanly to repo-backed runs

- Proposal Source: `4. Scope / In scope item 6` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:189-193`), `9.6 Delivery preflight extends the provider-platform baseline` (`...:716-733`), `10.2 Run Progress View enhancements` (`...:772-785`), `10.4 Worktree / diff affordances` (`...:810-819`), `14. Acceptance criteria / UI` (`...:1018-1024`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1429-1570`
  - `Chainworks Forge/Views/IdeaListView.swift:1573-1597`
  - `Chainworks Forge/Engine/DeliveryPreflightService.swift:26-100`
  - `/tmp/p007-audit.xcresult` — focused delivery/worktree tests passed
- Gap / Note: The run detail view already surfaces worktree/delivery sections and the provider-platform preflight extension exists, but these surfaces depend on mirrored `Run` fields that the current start path does not fully populate. This audit also did not close the repo-backed recovery/report/comparison proof on a real full-loop run.

### REQ-014 Proposal 007 sign-off proof is closed on current HEAD

- Proposal Source: `2. Product question this proposal must answer` (`docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md:96-120`), `12.3 Manual dogfood script` (`...:927-942`), `13. Testing strategy` (`...:946-991`), `14. Acceptance criteria / General and Product checkpoint` (`...:1032-1039`)
- Status: Not Verifiable
- Evidence Type: tests-run, tests-found, inference
- Evidence:
  - `/tmp/p007-compiler.xcresult` — focused compiler/workflow test pass
  - `/tmp/p007-audit.xcresult` — focused worktree/preflight/service tests pass
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:414-689` — integration/evidence-pack coverage exists in-tree but was not actually executed by the focused `xcodebuild` selector used for this audit
- Gap / Note: This audit did not produce an app-launched happy-path dogfood run, a blocked-release/non-happy-path run, a real exported evidence pack from that run, release-gate/final-receipts screenshots, or a full `xcodebuild build && xcodebuild test` green proof. The proposal-loop smoke non-regression claim is also not re-verified here.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `rg -n 'superseded|deprecated|replaced by|obsolete' 'docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md' 'docs/reviews'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <tmp> -resultBundlePath /tmp/p007-compiler.xcresult test -only-testing:'Chainworks ForgeTests/RunPlanCompilerTests'`
- `xcrun xcresulttool get test-results tests --path /tmp/p007-compiler.xcresult`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <tmp> -resultBundlePath /tmp/p007-audit.xcresult test -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests' -only-testing:'Chainworks ForgeTests/WorktreeProvisionerTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests'`
- `xcrun xcresulttool get test-results tests --path /tmp/p007-audit.xcresult`
- `rg -n "ReleaseOpsCoordinator|executeRelease\\(" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n "SourceContextBuilder|ImplementationDeliveryPreset" 'Chainworks Forge' 'Chainworks ForgeTests'`
- Focused file inspection on:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift`
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift`
  - `Chainworks Forge/Engine/DeliveryConfiguration.swift`
  - `Chainworks Forge/Engine/DeliveryPreflightService.swift`
  - `Chainworks Forge/Engine/SourceContextBuilder.swift`
  - `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Engine/OutputContractTemplates.swift`
  - `Chainworks Forge/Models/RunRepository.swift`
  - `Chainworks ForgeTests/RunPlanCompilerTests.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`

## Recommended Next Actions

- Wire repo-backed implementation execution to the actual provisioned worktree: update the execution context, working directory, and source-context path so `code_writer` and the review quartet operate on `run.worktreeRoot`, not the generic run workspace.
- Move `DeliveryConfiguration` freezing and run mirror-field persistence into the authoritative run-creation boundary, then ensure resume/report/export paths read the same frozen contract.
- Route state-11 release work through `ReleaseOpsCoordinator` and deterministic services only; remove the ability for simulated generic output templates to satisfy release receipts/manifests on the repo-backed path.
- Replace the scaffold behavior in `ConnectPublishService` with real archive/upload semantics or narrow the proposal if that work is intentionally deferred.
- Close sign-off proof with one happy-path repo-backed dogfood run, one non-happy-path blocked-release recovery run, an exported evidence pack from each, and a fresh full `xcodebuild build && xcodebuild test` green run.
