# Full MVP Delivery

Stable reference for the repo-backed delivery slice.

## Purpose

Chainworks Forge must be able to take an approved proposal all the way through implementation, implementation review, manual release, and evidence export from inside the app.

This document defines the current contract for that slice.

## Scope

This reference covers:

- the repo-backed `Full MVP Live` workflow,
- frozen delivery configuration for a run,
- dedicated writable worktree provisioning,
- repo-safety and explicit source-root boundaries,
- the implementation and review/refine loop,
- the manual release gate and deterministic release services,
- dogfood evidence export.

It does not redefine the surrounding baselines.
Those remain owned by:

- [live-provider-execution-slice.md](live-provider-execution-slice.md) for the live proposal-loop substrate,
- [operator-experience.md](operator-experience.md) for the operator shell, reports, recovery, and comparison,
- [provider-platform.md](provider-platform.md) for provider settings, diagnostics, and frozen provider truth,
- [project-workspace-contract.md](project-workspace-contract.md) for idea-owned workspace and frozen run root rules,
- [release-gate.md](release-gate.md) for deterministic native release execution and release receipt semantics,
- [live-workflow-map.md](live-workflow-map.md) for run-detail topology rendering,
- [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md) for the thin-UI GraphQL read contract.

## Core rules

The delivery slice is built around five rules:

1. one repo-backed run carries one frozen `DeliveryConfiguration`,
2. one repo-backed run gets one dedicated writable worktree,
3. write-capable implementation work happens only inside that worktree,
4. release side effects execute only after explicit human approval,
5. commit/push/archive/upload mechanics run through deterministic services, not free-form agent shelling.

## Canonical workflow

The canonical repo-backed workflow preset is [`examples/workflows/full-mvp-live.yaml`](../../examples/workflows/full-mvp-live.yaml).

It remains a 12-state flow with three explicit manual gates:

| State ID | Label | Purpose |
|---|---|---|
| `state_1_idea_received` | Idea received | normalize the idea and open run context |
| `state_2_proposal_drafted` | Proposal drafted | produce the first proposal artifacts |
| `state_3_initial_proposal_approval` | Human approval: initial proposal matches intent | manual gate before broad proposal review |
| `state_4_proposal_reviewed` | Proposal reviewed | parallel PO / UX / UI / architect review plus aggregation |
| `state_5_proposal_refined` | Proposal refined | revise until the proposal passes |
| `state_6_implementation_approval` | Human approval: proceed to implementation | manual gate before any repo-backed write work; rejection loops back to refinement |
| `state_7_implementation_started` | Implementation started | freeze proposal, provision worktree, create plan, start implementation |
| `state_8_implementation_continued` | Implementation continued until code work is seemingly complete | continue implementation until self-assessment says code-owned work is ready for review |
| `state_9_implementation_reviewed` | Implementation reviewed against proposal | docs/security review, auditor, pre-push review, aggregation |
| `state_8_implementation_continued` | Implementation continued | continue implementation until self-assessment status is complete, handoff_required, or blocked |
| `state_9_implementation_reviewed` | Implementation reviewed against proposal | docs/security review, auditor, prepush review, aggregation |
| `state_10_implementation_refined` | Implementation refined | apply findings and return to review until implementation passes |
| `state_11_manual_release` | Manual release | explicit release gate followed by deterministic release services |
| `state_12_workflow_complete` | Workflow complete | terminal state with durable receipts and report |

### Manual gates

The three manual gates are:

1. initial proposal intent check,
2. approval to begin implementation,
3. manual release approval.

Per P031-r18, these gates are **diagnostic-only** in the macOS UI. The UI renders the pending gate and provides diagnostic identifiers for external resolution via CLI or MCP tools. The product does not skip those gates in the repo-backed slice.

## Delivery configuration

`DeliveryConfiguration` is the authoritative per-run delivery contract.

It is frozen at run creation time and then read by:

- preflight,
- worktree provisioning,
- repo safety checks,
- release services,
- evidence export,
- resume/recovery.

The config carries:

- repository identity,
- repository root,
- base branch,
- worktree base path,
- target branch,
- release target identity,
- release mode,
- optional profile/sample-profile metadata.

`RepositoryProfile` is only a convenience producer for that same contract.
It is not a second source of truth.

## Worktree and repo-safety contract

Repo-backed implementation is isolated by `WorktreeProvisioner` and `RepoSafetyGuard`.

### Provisioning rules

- the configured repo root must exist,
- repository identity must match the frozen delivery contract,
- the base branch must exist before provisioning starts,
- the writable worktree must live under the configured worktree base path,
- the worktree branch is frozen from the run's target branch truth,
- one run cannot share a writable worktree with another run.

### Read and write boundaries

- write-capable agents operate only inside the provisioned worktree,
- release services read from that approved worktree state,
- read-only repo-backed stages use the frozen project/repo root, not an implicit current directory,
- artifact output remains outside the source tree under run storage,
- prompt packets explicitly tell agents to trust the provided project/worktree roots over any unexpected server cwd.

### Worktree Main Sync (P064)

Implementation worktrees may drift behind local main during long-running implementation or review loops.
- **Sync Trigger**: Sync can be triggered before implementation starts, before retries, before review, or on explicit operator request.
- **Barrier Protection**: Active sync acquires an exclusive mutation barrier that blocks all other worktree consumers (readers and writers).
- **Preservation**: Dirty work is preserved via a `chore: preserve run work before main sync` commit before any merge attempt.
- **Conflict Handling**: Merge conflicts result in an aborted merge and a resolver work item to handle the conflict manually.

Note: Main sync is currently in **Phase 0 contract freeze**.

## Implementation slice
...

Implementation begins only after the implementation-approval gate.

### Handoff into implementation

`state_7_implementation_started` is the handoff point where Chainworks:

- freezes the approved proposal,
- provisions the dedicated worktree,
- derives implementation plan/backlog,
- starts the first code-writing pass.

### Continue until implementation contract is complete

`code_writer` may loop in `state_8_implementation_continued` until the canonical
implementation self-assessment contract reports that code-writer-owned source and test
work is complete.

The loop reads `implementation_self_assessment_v2.implementation_complete` through the
canonical artifact contract layer documented in
[output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md).
It does not read legacy `implementation_self_assessment.seemingly_complete`.

For this loop, `implementation_complete` is scoped to code-writer-owned implementation
and verification work. `verification_green` records whether code-owned verification is
green, and `remaining_code_tasks` carries any code-owned blockers. Manual evidence,
release evidence, documentation-only follow-up, CloudKit signed-in smoke checks,
calendar/go-no-go decisions, and other operator/ops tasks are reported as `handoff_tasks`
or `known_risks`; they do not by themselves keep `code_writer` in the implementation
loop.
### Continue until status is ready

`code_writer` loops in `state_8_implementation_continued` until the `implementation_self_assessment_v2` reports a status that allows exiting the code loop:

- `complete`: no blocking code work or handoff work remains.
- `handoff_required`: code work and verification are complete, but non-code handoff tasks remain.
- `blocked`: code work is finished, but verification is not green (release readiness must be held).

`needs_code_fixes` or `invalid` statuses keep the `code_writer` in the loop.

**Migration and Retirement:**
The system supports a bounded migration from the legacy `v1` (based on `seemingly_complete`) to the `v2` structured contract:
- **Precedence**: Valid `v2` truth always wins over `v1`.
- **Fail-Closed**: If a `v2` generation exists but is invalid, the system records `status: invalid` and does not silently fall back to `v1`.
- **Retirement**: The `v1` fallback will be retired only when a release-cut gate reports zero active non-terminal `v1`-only runs of any age.

This loop is bounded by workflow counters.
Loop exhaustion pauses for a human instead of silently continuing forever.

**Suspicious-classification warnings:**
The domain parser emits warnings inside the assessment summary when it detects suspicious patterns:
- `suspicious_nonblocking_code_tasks_with_handoff`: handoff tasks exist while all remaining code tasks are marked non-blocking.
- `multiple_unknown_owner_class`: more than one handoff task uses the `unknown` owner class, requiring excessive human triage.
- `vague_evidence`: evidence strings are too short or use generic placeholders (e.g., "N/A", "TODO", "TBD").

These warnings are visible to reviewers and the operator but do not block the transition unless the overall status is `invalid`.

### Review order

`state_9_implementation_reviewed` keeps one explicit sequencing rule:

1. `security_checker` and `docs_guardian` run first,
2. `proposal_implementation_auditor` runs after `docs_report` exists,
3. `prepush_code_reviewer` runs after the audit phase,
4. orchestration aggregates the review outputs.

This order keeps `docs_report` available before the implementation auditor consumes it.

### Review outputs

The review/refine loop is expected to persist at least:

- `security_report`,
- `docs_report`,
- `docs_delta`,
- `audit_report`,
- `prepush_review_report`,
- `implementation_review_summary`,
- `implementation_self_assessment_v2`,
- `changed_files_manifest`,
- `tests_result_v1`.
- `tests_result`,
- `implementation_self_assessment_v2`.

### Refine loop

`state_10_implementation_refined` applies review findings and returns to implementation review until the workflow-level implementation success conditions are met.

The slice does not treat "looks good locally" as enough.
The exit condition is canonical artifact-backed review status. The implementation gate
reads normalized `audit_report`, `security_report`, `prepush_review_report`,
`docs_report`, and `tests_result_v1.status` truth from the active artifact index.

## Manual release

Release remains explicit.
The app does not autonomously push or distribute code without the manual release gate being granted.

### Release gate

`ReleaseGateView` is the operator-facing **diagnostic** surface for the final gate.
It must show enough context to make an informed decision for execution via an external workflow:

- proposal and workflow context,
- review artifact availability,
- repo identity, branch, and worktree,
- release target,
- quick actions around diff/worktree inspection.

**Verification Truth:**
When `implementation_self_assessment_v2` is available, `ReleaseGateView` uses its `verification_green` signal instead of legacy test results.
If the status is `blocked`, a high-visibility warning row is shown at the top of the Change Summary section.

**Decision Execution:**
Buttons for `Approve` or `Reject` are replaced with diagnostic banners or technical details (run ID, branch name, target) for use in external workflows.

### Deterministic release sequence

After approval, `ReleaseOpsCoordinator` drives:

1. commit and push through `GitReleaseService`,
2. archive/build/upload through `ConnectPublishService`.

Agents may recommend release, but they do not improvise git/archive/upload mechanics.
The detailed release contract now lives in [release-gate.md](release-gate.md).

### Partial failure semantics

If commit/push succeeds and archive/upload fails:

- the run keeps the already-produced receipts,
- the run becomes blocked/operator-visible,
- there is no hidden rollback,
- recovery happens through the existing operator shell and recovery surfaces.

### Release targets

The baseline release modes are:

- `sandbox`,
- `staging`.

Production-by-default targets are intentionally excluded from this slice.

### UI ownership

Repo-backed delivery does not introduce a separate shell.

Canonical owner path:

1. `Ideas` and `Start Run` (Removed or replaced with diagnostic placeholders in P031),
2. run progress / run detail (GraphQL read-only),
3. approval gate and release gate (Diagnostic-only),
4. report / receipts / evidence export (Read-only artifacts).

**Implementation assessment UI:**
`WorkflowRunProgressView` includes a **read-only** `ImplementationSelfAssessmentPanel` in the Decision Context section. This panel provides a structured view of the `implementation_self_assessment_v2` artifact, showing status, verification greenness, remaining code tasks, and handoff tasks with owner icons.

The key surfaces are:

- repo-backed diagnostic preflight,
- run progress with repo/worktree-aware context and implementation assessment summary,
- `ReleaseGateView` (Diagnostic),
- report and evidence export,
- existing recovery/comparison surfaces (Read-only) from the operator baseline.

## Dogfooding and evidence export

The slice includes a dogfood-oriented preset and evidence export path.

### Dogfood preset

`Full MVP Live` is the first opinionated repo-backed preset intended for real internal sessions against a safe repository target.

### Evidence export

`EvidencePackBuilder` exports a dogfood evidence pack for a repo-backed run.
The pack includes:

- run metadata,
- frozen delivery configuration,
- delivery preflight,
- copied artifacts,
- named delivery artifacts,
- stage summary,
- agent execution detail,
- screenshot checklist.

### Evidence expectations

The slice is only fully credible when both of these exist:

1. one happy-path repo-backed run,
2. one non-happy-path repo-backed run with preserved recovery context.

## Non-goals

This slice does not add:

- concurrent writable agents in the same worktree,
- fully autonomous release,
- automatic rollback,
- multi-repo orchestration,
- background/cloud execution,
- production-by-default release targets,
- multi-user delivery coordination.
