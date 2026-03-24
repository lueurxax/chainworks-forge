# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding

| Field | Value |
|---|---|
| Date | 2026-03-23 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 001 (foundation, now stabilized as documentation), Proposal 002 (workflow execution engine), Proposal 004 (live provider proposal slice), Proposal 005 (operator experience), Proposal 006 (provider expansion, settings, diagnostics, pilot readiness) |
| Adjacent work | Proposal 003 (Forge Steward) remains valuable but is **not** on the critical path for first end-to-end dogfooding |
| Goal | Complete the first real repo-backed MVP path: idea → proposal → implementation → implementation review → manual release → durable receipts, with enough safety and evidence to dogfood the product on a real repository without improvising the orchestration by hand. |

---

## 1. Context

Proposal 002 gave Chainworks its execution spine:
- `RunPlanCompiler`
- `ExecutionService`
- `WorkflowOrchestrator`
- `ArtifactManager`
- approval and resume semantics
- explicit `RunWorkspace` and workspace-isolation rules

Proposal 004 then proved the first **live** slice, but it was intentionally narrow:
- real provider-backed execution,
- real artifacts,
- real approval,
- but only for the **proposal loop**.

Proposal 005 turned the app into something calmer to operate:
- reports,
- recovery flows,
- comparison,
- notifications,
- better artifact ergonomics.

Proposal 006 made the provider layer viable in practice:
- multiple provider families,
- settings and secrets,
- preflight diagnostics,
- immutable provider bindings,
- first-run / pilot surfaces.

That is a lot of important groundwork.

What is still missing is the part that makes the product match its own promise:

> Can Chainworks take an approved proposal, create and isolate a writable implementation workspace, let the code-writing/review/release agents do real work against a real repository, pause for explicit human judgment at release time, and finish with durable receipts?

Right now, without that slice, the app can feel impressive in a proposal review demo and still leave the operator doing the hardest, messiest part by hand:
- opening a worktree,
- applying the proposal to code,
- running the review quartet,
- collecting the green lights,
- committing,
- pushing,
- building,
- uploading,
- and then explaining afterward what happened.

Proposal 007 closes that gap.

### Important framing

Proposal 007 does **not** change Proposals 005 or 006.
It consumes them as-is.

That means:
- operator report/recovery/comparison surfaces from Proposal 005 stay valid,
- provider/platform/settings/diagnostics surfaces from Proposal 006 stay valid,
- Proposal 007 is about **using those surfaces to complete the first full delivery path**, not replacing them.

### 1.1 Explicit handoff from Proposal 006

Nothing provider/platform-specific was removed from Proposal 006.
But Proposal 006 deliberately stops at provider readiness for the current control-plane baseline.
Any capability that becomes repo-backed, writable, release-aware, or delivery-specific is owned here in Proposal 007.

That handoff includes:
- repo-backed start presets built on top of Proposal 006 provider/settings resolution,
- delivery-specific preflight that extends Proposal 006 with repo identity, branch, worktree, git auth, and release-target checks,
- repository profile and target-repository selection for dogfood runs,
- writable worktree provisioning and recovery semantics,
- release-target configuration and release-gate context,
- repo-backed evidence/support export for full delivery runs.

Rule of thumb:

> If a feature can be completed without touching a real repository or release target, it belongs in Proposal 006.
> If it requires a writable repo, a dedicated worktree, commit/push, archive/upload, or delivery-specific recovery, it belongs in Proposal 007.

---

## 2. Product question this proposal must answer

Proposal 007 succeeds only if one engineer can do the following **from inside the app** against a real repository-backed target:

1. Create an idea
2. Run the real proposal loop
3. Approve the proposal
4. Enter a dedicated writable implementation worktree
5. Let the code writer make real changes
6. Run real implementation review against the approved proposal
7. Iterate until implementation review status is green
8. Reach a real manual release gate
9. Approve release
10. See deterministic commit/push/distribute services run
11. End with a completed run, readable report, and durable receipts

If the engineer still has to leave the app and manually glue together the repo/release half of the process, then the MVP still is not truly in the hands.

### Definition of done

Proposal 007 is only done when all of the following are true at once:

1. the 10-state workflow is executable end-to-end in a repo-backed mode;
2. write-capable execution is isolated to one dedicated worktree per run;
3. release side effects happen only behind explicit manual approval and deterministic services;
4. the system can produce one happy-path evidence pack and one non-happy-path evidence pack;
5. the operator can recover from a blocked release without guessing.

---

## 3. What we build

Two tightly scoped layers.

### Layer I: Repo-Backed Delivery Runtime

| Component | Responsibility |
|---|---|
| **WorktreeProvisioner** | Create and persist a dedicated writable implementation worktree for one run |
| **RepoSafetyGuard** | Enforce repo identity, base branch, path boundaries, and write scope |
| **SourceContextBuilder** | Materialize the code context the writing/review agents need without relying on hidden cwd state |
| **ImplementationDeliveryPreset** | Compile a repo-backed executable workflow from the current catalog and workflow fixtures |
| **ReleaseOpsCoordinator** | Drive commit → push → archive → distribute after approval |
| **GitReleaseService** | Deterministic service for commit/push; no free-form agent shelling for release mechanics |
| **ConnectPublishService** | Deterministic service for build/archive/upload; no source edits |
| **DeliveryReceiptBuilder** | Produce structured receipts, diff summaries, and release manifests |

### Layer J: Dogfooding Pack

| Component | Responsibility |
|---|---|
| **Full MVP Live Workflow** | First executable 10-state repo-backed workflow preset |
| **Dogfood Start Preset** | Opinionated safe defaults for repo, workflow, providers, and release target |
| **Evidence Pack Builder** | Export the screenshots, reports, receipts, and support bundle needed to review a dogfood session |
| **Sample Repo Profile** | A small repeatable repository target for first live runs |
| **Release Gate Summary Surface** | One place to understand exactly what is about to be released |

---

## 4. Scope

### In scope

1. The first **repo-backed** end-to-end execution path for the existing 10-state workflow model:
   - idea received
   - proposal drafted
   - proposal reviewed
   - proposal refined
   - implementation started
   - implementation continued
   - implementation reviewed
   - implementation refined
   - manual release
   - workflow complete

2. A dedicated writable worktree per run for:
   - `code_writer`
   - `docs_guardian` when it is allowed to edit docs
   - release services reading from the already-approved worktree

3. Real execution of the implementation/release-side agents:
   - `code_writer`
   - `proposal_implementation_auditor`
   - `security_checker`
   - `prepush_code_reviewer`
   - `docs_guardian`
   - `commit_and_push_to_github`
   - `build_archive_and_push_connect`

4. Manual release gating with deterministic side-effect services.

5. A dogfooding-ready workflow preset and evidence export.

6. Repo-backed extensions of Proposal 006 surfaces, specifically:
   - delivery preflight in addition to provider preflight,
   - repo/profile selection for full runs,
   - release-target selection and release-context summaries,
   - dogfood-oriented onboarding for the first full repository-backed session.

### Out of scope

1. Multiple write-capable agents editing the same worktree concurrently.
2. Autonomous release with no human gate.
3. Automatic rollback after push/upload.
4. Multi-repo orchestration.
5. Background/cloud execution or Temporal migration.
6. Forge Steward feedback loops mutating the workflow automatically.
7. Production-by-default release targets.
8. Team/multi-user coordination surfaces.

### Safety default

Proposal 007 defaults to **sandbox or staging release targets**, not production.
The point is to feel the full loop in the body without inviting unnecessary blast radius.

---

## 5. Canonical live workflow for Proposal 007

Proposal 004 introduced `proposal-loop-live.yaml` as the fast smoke path.
Proposal 007 adds a new preset:

`examples/workflows/full-mvp-live.yaml`

This is the first **repo-backed** dogfood workflow.

### 5.1 State map

| State ID | Label | Owner | What happens |
|---|---|---|---|
| `state_1_idea_received` | Idea received | `lead_orchestrator` | Normalize the idea and prepare brief/context |
| `state_2_proposal_drafted` | Proposal drafted | `proposal_writer` | Produce initial proposal artifacts |
| `state_3_proposal_reviewed` | Proposal reviewed | `lead_orchestrator` | Parallel PO / UX / UI / Architect review, then aggregate |
| `state_4_proposal_refined` | Proposal refined | `proposal_writer` | Refine until proposal score target passes |
| `state_5_implementation_started` | Implementation started | `lead_orchestrator` | Freeze approved proposal, provision worktree, generate plan/backlog, start initial implementation |
| `state_6_implementation_continued` | Implementation continued until seemingly complete | `code_writer` | Continue work until self-assessment says “seemingly complete” |
| `state_7_implementation_reviewed` | Implementation reviewed against proposal | `lead_orchestrator` | Security/docs review first, then auditor, then pre-push review, then aggregate |
| `state_8_implementation_refined` | Implementation refined | `code_writer` | Apply findings, sync docs, loop until status is `Implemented` |
| `state_9_manual_release` | Manual release | `lead_orchestrator` | Human approval, then deterministic commit/push and archive/distribute |
| `state_10_workflow_complete` | Workflow complete | `lead_orchestrator` | Terminal state with final receipts/report |

### 5.2 Why a separate live workflow preset exists

The original canonical workflow is a strong design baseline, but Proposal 007 needs one executable variant that is extra strict about:
- worktree preparation,
- docs artifact availability,
- release receipt sequencing,
- and operator-facing release context.

That means `full-mvp-live.yaml` can and should be slightly more explicit than the earlier abstract design baseline.

### 5.3 Important workflow refinement for consistency

There is one practical rule Proposal 007 should lock:

> `docs_report` must exist before the implementation auditor consumes it.

For the repo-backed live preset, `state_7_implementation_reviewed` should therefore run in this order:

1. **parallel**
   - `security_checker`
   - `docs_guardian`

2. **then**
   - `proposal_implementation_auditor`
   - `prepush_code_reviewer`
   - `lead_orchestrator` (aggregate)

This fixes a quiet but important execution problem:
the auditor should not depend on a docs artifact that has not yet been produced.

---

## 6. Architecture

```text
SwiftUI App
  -> ExecutionService
    -> WorkflowOrchestrator
      -> WorktreeProvisioner / RepoSafetyGuard
      -> Provider-backed AgentExecutor (from Proposal 004 / 006)
      -> ReleaseOpsCoordinator
         -> GitReleaseService
         -> ConnectPublishService
      -> ArtifactManager / ArtifactStorage
      -> Approval + Recovery + Reports (from Proposal 005)
```

### 6.1 Control plane stays in the app

Proposal 007 does not move orchestration into provider runtimes.

The app remains responsible for:
- state transitions,
- run snapshots,
- artifact indexing,
- worktree identity,
- recovery,
- approval gates,
- and operator surfaces.

### 6.2 Providers remain execution substrates

The provider layer from Proposals 004 and 006 remains responsible for:
- model execution,
- tool use,
- structured output,
- receipts,
- provider health and configuration.

### 6.3 Release services are deterministic

Release is different from proposal writing or code drafting.
For release mechanics, Proposal 007 draws a hard line:

> Agents may decide **that** release should happen, but they do not free-form the commit/push/archive/upload mechanics.

Those mechanics go through deterministic app-controlled services.

That makes the most dangerous part of the workflow:
- easier to reason about,
- easier to test,
- easier to recover,
- and much less likely to leak across workspaces or repositories.

---

## 7. Dedicated worktrees and repo safety

## 7.1 Core rule

> One run = one dedicated writable implementation worktree.

Not:
- one chat,
- one project window,
- one shared agent pool.

One **run**.

### 7.2 Worktree identity

`WorktreeProvisioner` creates a dedicated worktree when the run crosses from approved proposal into implementation.

Suggested naming:

```text
{configuredWorktreeBase}/cw-{ideaSlug}-{runShortID}/
```

Example:

```text
.chainworks/worktrees/cw-auth-flow-a1b2c3/
```

### 7.3 Persisted metadata

Proposal 007 should persist enough worktree/repo data to recover and inspect the run cleanly.

Recommended additions to `Run`:

```swift
// Added to Run
var worktreeRoot: String?
var repoIdentifier: String?      // stable logical repo id or path hash
var baseBranch: String?
var baseRevision: String?        // commit SHA used when the worktree was created
var targetBranch: String?        // release branch / push target
var releaseTargetID: String?     // sandbox/staging destination id
var releaseMode: String?         // sandbox | staging
```

Recommended additions to `AgentExecution`:

```swift
// Added to AgentExecution
var repoRevisionBefore: String?
var repoRevisionAfter: String?
var consumedArtifactIDsJSON: Data?   // if not already added in Proposal 005 implementation
```

### 7.4 Provisioning rules

`WorktreeProvisioner` must:

1. verify the source repository identity;
2. verify base branch exists;
3. record base revision;
4. create the worktree in the configured base path;
5. ensure the path is inside the allowed worktree root;
6. return a frozen `worktreeRoot` written onto the run.

### 7.5 No shared write worktrees

Proposal 007 explicitly forbids:
- multiple active runs sharing one writable worktree,
- two concurrent write-capable agents in the same worktree,
- any writing outside the run’s `worktreeRoot`.

### 7.6 Read-only review agents

Review agents do not need write privileges:
- `proposal_implementation_auditor`
- `security_checker`
- `prepush_code_reviewer`

They can operate against:
- the frozen approved proposal artifacts,
- the current worktree snapshot,
- generated diff/test artifacts,
- and read-only repo inspection tools.

### 7.7 Path boundary enforcement

Before any file operation or tool call:
- target path must be under `workspaceRoot` or `worktreeRoot`,
- release services must refuse any repo root mismatch,
- a violation blocks the run immediately.

This is the repo-backed extension of the workspace-isolation rule already documented earlier.

---

## 8. Implementation slice

## 8.1 Handoff from approved proposal

When the proposal passes its approval gate, the system enters `state_5_implementation_started`.

That stage should do three things in order:

1. **Freeze the approved proposal**
   - `approved_proposal`
   - `implementation_plan`
   - `implementation_backlog`
   - `run_state`

2. **Provision worktree**
   - create dedicated writable worktree,
   - record repo/base metadata,
   - optionally emit a `worktree_manifest` artifact.

3. **Start initial implementation**
   - `code_writer` makes the first real pass,
   - produces:
     - `implementation_progress`
     - `implementation_self_assessment`
     - `changed_files_manifest`
     - `tests_result`

## 8.2 Continue until seemingly complete

`state_6_implementation_continued` is the first real implementation loop.

It is intentionally simple:

- `code_writer` keeps working
- tests keep being run
- `implementation_self_assessment.seemingly_complete` is the gate
- loop budget prevents endless optimistic circling

### Why this matters

Without this stage, the system tends to jump too early into review.
You end up paying for a review quartet to tell you the code is only half-shaped.
That is expensive and demoralizing.

## 8.3 Implementation reviewed against proposal

This is where the repo-backed slice becomes real.
The app is no longer only moving text artifacts around.
It is comparing a live code change against the approved intent.

Recommended execution order for `state_7_implementation_reviewed`:

### Parallel phase
- `security_checker`
- `docs_guardian`

### Sequential phase
- `proposal_implementation_auditor`
- `prepush_code_reviewer`
- `lead_orchestrator` (aggregate)

### Why this order

- security and docs can inspect the current code snapshot independently;
- the auditor benefits from a current docs view;
- the pre-push reviewer benefits from audit + security outputs;
- the lead agent then aggregates all signals into `implementation_review_summary`.

### Required artifacts

At minimum, this stage should persist:

- `security_report`
- `docs_report`
- `audit_report`
- `prepush_review_report`
- `implementation_review_summary`
- `orchestrator_summary`

## 8.4 Implementation refined

If review status is not `Implemented`, the run enters `state_8_implementation_refined`.

Recommended sequence:

1. `code_writer`
   - apply findings from:
     - `audit_report`
     - `security_report`
     - `prepush_review_report`
     - `implementation_review_summary`

2. `docs_guardian`
   - sync documentation after the new code pass,
   - emit:
     - `docs_report`
     - `docs_delta`

Then loop back to `state_7_implementation_reviewed`.

### Practical rule

The code writer should not silently absorb release mechanics or documentation ownership.
The roles stay narrow:
- code changes belong to `code_writer`,
- doc alignment belongs to `docs_guardian`,
- release mechanics belong to deterministic services invoked through the release stage.

---

## 9. Manual release and deterministic side effects

## 9.1 Release must remain explicit

`state_9_manual_release` is not a polite formality.
It is the point where the app crosses from “thinking and editing” into “changing remote state”.

Proposal 007 keeps that boundary hard.

The release gate must summarize:
- what feature/proposal is being released,
- what changed in code,
- what changed in docs,
- current implementation review status,
- security/audit/prepush outcome,
- tests result,
- target branch,
- target release destination,
- current spend or explicit unavailable/estimated spend.

## 9.2 Release step sequence

After approval, release runs in deterministic sequence:

1. `commit_and_push_to_github`
   - via `GitReleaseService`
   - outputs:
     - `release_manifest`
     - `git_push_receipt`

2. `build_archive_and_push_connect`
   - via `ConnectPublishService`
   - inputs:
     - `git_push_receipt`
     - `release_manifest`
   - outputs:
     - `release_bundle_manifest`
     - `connect_upload_receipt`

## 9.3 Service contract

### GitReleaseService

Inputs:
- `worktreeRoot`
- `repoIdentifier`
- `targetBranch`
- `approved_proposal`
- `implementation_review_summary`
- `docs_report`
- `prepush_review_report`

Outputs:
- commit SHA
- remote/branch
- release manifest
- status
- failure reason if any

Rules:
- no source edits,
- no staging arbitrary extra files outside approved worktree state,
- no implicit branch guessing,
- no push if gate not approved.

### ConnectPublishService

Inputs:
- `worktreeRoot`
- `git_push_receipt`
- `release_manifest`
- `releaseTargetID`
- `releaseMode`

Outputs:
- bundle/checksum
- artifact ID
- destination
- status
- failure reason if any

Rules:
- no source edits,
- deterministic build inputs,
- explicit target only,
- checksum recorded always.

## 9.4 Partial failure semantics

A particularly ugly real-world case is:
- commit/push succeeds,
- archive/upload fails.

Proposal 007 should not pretend that never happens.

When it does:
- receipts remain persisted,
- run becomes `blocked`,
- operator sees exactly which sub-step succeeded,
- recovery happens through the operator surfaces defined in Proposal 005,
- the system does **not** invent a hidden rollback.

### Recovery rule

Proposal 005 already set the tone:
side-effect retries should re-enter through an approval boundary or a cloned run, not through a silent direct rerun.

Proposal 007 respects that.
On partial release failure, the system returns the operator to a release-context recovery path, not to a blind “retry publish” button with no context.

## 9.5 Default release target modes

Proposal 007 should support at least:

```swift
enum ReleaseMode: String, Codable {
    case sandbox
    case staging
}
```

Production is intentionally excluded from the initial dogfood slice.

## 9.6 Delivery preflight extends Proposal 006

Proposal 006 owns provider/platform diagnostics.
Proposal 007 must add the missing delivery checks before a repo-backed run can cross into implementation or release.

At minimum, delivery preflight must verify:
- target repository identity and expected root,
- selected base branch exists,
- worktree base path is writable and inside the configured allowed root,
- git auth/push target is usable for the selected branch,
- selected release target is valid for the chosen `ReleaseMode`,
- no repo-safety contract violation exists between the run and the chosen repository.

This is intentionally an extension of Proposal 006, not a replacement for it:
- provider health remains Proposal 006 territory,
- repo/release readiness becomes Proposal 007 territory.

---

## 10. UI surfaces

Proposal 005 already added the operator spine.
Proposal 007 only adds the extra surfaces needed for the repo-backed slice.

## 10.1 Dogfood Start Run preset

A new start surface or preset should make it easy to launch the full flow without assembling options manually every time.

Required inputs:
- workflow preset: `Full MVP Live`
- repo/workspace target
- release mode: sandbox or staging
- provider binding summary (already available from Proposal 006)
- preflight summary

This surface is the point where Proposal 006 hands off to Proposal 007 in the UI:
- provider settings and provider diagnostics come from Proposal 006,
- repo target, release target, and delivery safety context are added here by Proposal 007.

Suggested summary block:

```text
Workflow: Full MVP Live
Repo: /path/to/repo
Release target: Sandbox
Providers: proposal/review/implementation/release bindings resolved
Safety: dedicated worktree, manual release gate, deterministic release services
```

## 10.2 Run Progress View enhancements

Proposal 007 should enrich the existing run screen with a few repo-specific signals:

- current worktree path
- changed file count
- diff stat summary
- latest test result status
- current implementation loop iteration
- latest implementation review score/status
- release target and release gate status

The view should still stay calm.
It should not become a raw git console.

## 10.3 Release Gate View

This is the one new operator surface that matters most.

It should show, above the fold:
- proposal summary
- review summary status
- changed files / diff stat
- tests result
- security summary
- audit summary
- docs summary
- target branch
- release destination

Quick actions:
- open proposal
- open diff summary
- open docs delta
- open receipts/report
- approve
- reject

## 10.4 Worktree / diff affordances

Proposal 005’s Artifact Inspector V2 already covers artifact viewing.
Proposal 007 should simply add the repo-backed shortcuts:
- open worktree in Finder
- reveal diff summary
- open changed files manifest
- reveal release manifest

No separate giant repo browser is needed for the first dogfood slice.

---

## 11. Small DSL and catalog deltas required by Proposal 007

Proposal 007 should keep changes surgical.

### 11.1 Parse `approval_policy`

The existing release state carries release-specific approval meaning.
Proposal 007 should extend the DSL model to preserve:

```yaml
approval_policy: manual_release
```

Recommended addition:

```swift
// Added to WorkflowState
let approvalPolicy: String?

enum CodingKeys: String, CodingKey {
    case approvalPolicy = "approval_policy"
    // existing keys...
}
```

This gives the UI enough context to render a tailored release gate instead of a generic approval sheet.

### 11.2 Add `full-mvp-live.yaml`

Do not overwrite the fast smoke workflow from Proposal 004.
Keep both:

- `proposal-loop-live.yaml` — fast real smoke test
- `full-mvp-live.yaml` — first repo-backed dogfood workflow

### 11.3 Keep role bindings explicit

Proposal 007 should continue to resolve the concrete agents from the catalog rather than creating hidden hardcoded role fallbacks.

That means the live workflow preset still uses the existing catalog entries:
- `lead_orchestrator`
- `proposal_writer`
- `proposal_reviewer_product_owner`
- `proposal_reviewer_ux`
- `proposal_reviewer_ui`
- `proposal_reviewer_architect`
- `code_writer`
- `proposal_implementation_auditor`
- `security_checker`
- `prepush_code_reviewer`
- `docs_guardian`
- `commit_and_push_to_github`
- `build_archive_and_push_connect`

---

## 12. Dogfooding pack

Proposal 007 is not only about execution.
It is about getting to the first serious lived experience fast enough that the product starts teaching back.

## 12.1 Sample repo profile

The first dogfood target should be:
- small enough to finish in one sitting,
- real enough to exercise code + docs + tests + release,
- stable enough that comparison is meaningful.

Suggested qualities:
- single repo
- clear test command
- at least one docs surface
- no exotic build chain
- safe sandbox/staging release target

## 12.2 Evidence pack builder

Every full dogfood run should be exportable as one pack containing:

- run report
- pinned artifacts
- proposal draft
- implementation review summary
- docs report / docs delta
- diff summary
- git push receipt
- connect upload receipt
- support bundle from Proposal 006
- screenshot checklist

### Suggested required screenshots

Happy path:
1. Start Run preset
2. Proposal approved
3. Implementation review green
4. Manual release gate
5. Completed run with receipts

Non-happy path:
1. blocked implementation review or blocked release
2. recovery sheet / release gate re-entry
3. final recovered or cancelled state

## 12.3 Manual dogfood script

One engineer should be able to run this in a single focused session:

1. Launch app
2. Choose sample repo
3. Create idea
4. Start `Full MVP Live`
5. Approve proposal
6. Watch worktree provision
7. Watch implementation loop
8. Review implementation summary
9. Approve manual release
10. Inspect final receipts and exported evidence pack

If this cannot be repeated with low drama, the system is still too theoretical.

---

## 13. Testing strategy

## 13.1 Unit tests

### Worktree / repo safety
- `testWorktreeProvisionerCreatesUniqueWorktreePerRun()`
- `testWorktreeProvisionerPersistsBaseRevision()`
- `testRepoSafetyGuardRejectsPathOutsideWorktree()`
- `testRepoSafetyGuardRejectsRepoIdentityMismatch()`
- `testNoConcurrentWritableAgentUsesSharedWorktree()`

### Workflow / implementation loop
- `testFullMVPLiveWorkflowCompiles()`
- `testImplementationState5ProvisionsWorktreeBeforeCodeWriter()`
- `testImplementationLoopStopsWhenSeeminglyComplete()`
- `testImplementationReviewOrderGuaranteesDocsReportBeforeAudit()`
- `testImplementationRefineLoopReentersReview()`

### Release ops
- `testManualReleaseRequiresApproval()`
- `testGitReleaseServiceProducesReceiptAndManifest()`
- `testConnectPublishServiceProducesReceiptAndBundleManifest()`
- `testPartialReleaseFailureBlocksRunWithReceiptsPreserved()`

## 13.2 Integration tests

### Safe local integration
- `testFullMVPLiveRunAgainstSampleRepo()`
- `testBlockedReleaseReturnsToRecoverySurface()`
- `testResumeDuringImplementationStageRestoresWorktreeContext()`
- `testRejectManualReleaseCancelsRunWithoutSideEffects()`

### Env-gated live smoke tests
- `testSandboxPushSmoke()`
- `testSandboxConnectUploadSmoke()`
- `testFullDogfoodRunSmoke()`

## 13.3 Evidence-based review requirement

Proposal 007 should not be signed off on code/fixtures alone.

Required review evidence:
- one full happy-path dogfood run,
- one non-happy-path run,
- exported evidence pack,
- screenshots for the release gate and final receipts.

---

## 14. Acceptance criteria

### Runtime / worktree
- [ ] A dedicated writable worktree is provisioned and persisted on the run before the first implementation write
- [ ] No write-capable action can target a path outside `worktreeRoot` / `workspaceRoot`
- [ ] Two concurrent runs cannot share one writable worktree
- [ ] Repo identity and base revision are recorded for the run

### Workflow
- [ ] `full-mvp-live.yaml` compiles into a valid executable plan
- [ ] The 10-state workflow can move from approved proposal into repo-backed implementation
- [ ] `state_7_implementation_reviewed` produces `docs_report`, `audit_report`, `security_report`, `prepush_review_report`, and `implementation_review_summary`
- [ ] `state_8_implementation_refined` can loop back into review until `status == Implemented`
- [ ] `state_9_manual_release` blocks on explicit human approval

### Release
- [ ] Release side effects execute only through deterministic services
- [ ] Commit/push produces `git_push_receipt` and `release_manifest`
- [ ] Archive/distribute produces `connect_upload_receipt` and `release_bundle_manifest`
- [ ] Partial release failure preserves receipts and returns the run to an operator-visible blocked state
- [ ] Default release modes are sandbox/staging, not production

### UI / operator experience
- [ ] Start Run supports the `Full MVP Live` preset
- [ ] Run Progress view shows worktree-aware implementation progress
- [ ] Release Gate View presents enough context for an informed approval
- [ ] Existing report/recovery/comparison surfaces from Proposal 005 work for repo-backed runs
- [ ] Provider diagnostics/preflight from Proposal 006 apply cleanly to the full workflow

### Dogfooding
- [ ] A single engineer can complete a full happy-path run on a sample repo from inside the app
- [ ] A non-happy-path run (blocked implementation review or blocked release) is recoverable without guessing
- [ ] Evidence Pack Builder exports a complete dogfood packet
- [ ] Proposal-loop-only smoke workflow from Proposal 004 still works unchanged

### General
- [ ] No regressions in Proposals 002, 004, 005, or 006
- [ ] `xcodebuild build && xcodebuild test` green

### Product checkpoint (PROD-PA-007)
- [ ] One engineer can go from idea creation to repo-backed completed release candidate in a sandbox/staging target, fully inside the app, in under 25 minutes on the sample repo
- [ ] One non-happy-path run is captured and recovered (or intentionally cancelled) with preserved receipts and clear operator context
- [ ] The product now supports a believable full-loop dogfood session, not only a proposal demo

---

## 15. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| ARCH-067 | One run = one dedicated writable worktree | Prevents cross-run contamination and makes recovery/reasoning possible |
| ARCH-068 | No concurrent write-capable agents in the same worktree | Lowers risk and keeps repo state explainable |
| ARCH-069 | Release mechanics run through deterministic services, not free-form agent shelling | Safer, more testable, easier to recover |
| ARCH-070 | `full-mvp-live.yaml` is a separate repo-backed dogfood preset; `proposal-loop-live.yaml` remains the fast smoke path | Preserve fast feedback while enabling full-loop testing |
| ARCH-071 | `docs_report` must exist before audit aggregation in the repo-backed live preset | Removes input ambiguity in the first implementation review cycle |
| ARCH-072 | Default release targets are sandbox/staging only | Keep the first dogfood slice honest but safe |
| ARCH-073 | Partial release failure returns to blocked/operator recovery, not hidden rollback | Trustworthy receipts beat magical recovery |
| ARCH-074 | Proposal 003 remains adjacent, not critical path; Proposal 007 emits the artifacts and telemetry Steward will later benefit from | Keep first full-loop delivery moving |

---

## 16. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Worktree leakage or repo mismatch | Wrong repository gets modified or reviewed | Persisted repo identity, path guards, one worktree per run, deterministic provisioning |
| Code writer loops too long or burns too much budget | Dogfood sessions become exhausting instead of illuminating | Tight loop budgets, explicit self-assessment gate, visible elapsed/cost from Proposals 005/006 |
| Docs flow remains awkward | Review stage blocks on missing or stale docs | Produce docs before audit in `state_7`, sync docs again in `state_8` after refinements |
| Release services are the first truly dangerous side effects | Loss of trust after one bad push/upload | Manual gate, sandbox/staging default, deterministic services, preserved receipts |
| Sample repo hides pain that real repos have | False confidence | Keep sample repo small for first proof, then add one second, messier repo after sign-off |
| Full-loop run is technically correct but emotionally clumsy | Product still feels like scaffolding | Dogfood preset, release gate summary, evidence pack, calm operator surfaces |
| Proposal 006 provider matrix adds too many moving parts | Hard-to-debug live failures | Use one known-good preset for dogfood first, mixed-provider breadth second |

---

## 17. Execution plan

| Day | Deliverable |
|---|---|
| Day 1 | `full-mvp-live.yaml`, `approval_policy` parsing, model additions for worktree/repo metadata |
| Day 2 | `WorktreeProvisioner` + `RepoSafetyGuard` + tests |
| Day 3 | Implementation handoff/state 5 integration + code-writer repo context |
| Day 4 | Implementation review/refine loop wiring, including docs-before-audit ordering |
| Day 5 | `ReleaseOpsCoordinator` + `GitReleaseService` + `ConnectPublishService` |
| Day 6 | Release Gate UI + dogfood preset + worktree-aware progress affordances |
| Day 7 | Evidence Pack Builder + sample repo profile + smoke runs |
| Day 8 | Happy-path / non-happy-path dogfood passes + polish |

---

## 18. What Proposal 007 enables

After Proposal 007, Chainworks stops being only:
- a definition viewer,
- a workflow engine,
- a proposal-loop demo,
- or a polished operator shell.

It becomes a believable **full-loop local control plane** for one engineer:

- ideas become approved proposals,
- approved proposals become code in an isolated worktree,
- implementation is reviewed against intent,
- release is explicit and traceable,
- and the whole session leaves behind receipts, reports, and evidence.

That is the first moment the product can really be dogfooded in the way it was imagined.

---

## 19. Likely next steps after Proposal 007

Not part of this proposal, but the sequence becomes clearer after it lands:

1. **Dogfood Learnings / Hardening**
   - fix rough edges found in the first full sessions
   - add one messier real-world repo target

2. **Forge Steward Activation**
   - Proposal 003 finally becomes high-leverage once enough real full-loop runs exist

3. **Backend Extraction / Temporal Phase**
   - only after the local full-loop is stable and worth scaling

---

## 20. Final recommendation

Do not make Proposal 007 bigger than this.

The temptation will be:
- more automation,
- more provider cleverness,
- more rollout logic,
- more “smart” release behavior.

Resist it.

What the product needs next is one honest, repeatable, repo-backed full loop that a tired engineer can run, inspect, recover, and trust.

That is the proposal.
