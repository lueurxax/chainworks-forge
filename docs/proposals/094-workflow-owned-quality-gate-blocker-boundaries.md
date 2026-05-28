# Proposal 094: Workflow-Owned Quality-Gate Blocker Boundaries

| Field | Value |
|---|---|
| Date | 2026-05-26 |
| Status | Draft / Rewrite |
| Author | Chainworks |
| Rewrites | `094-orchestrated-quality-gate-blocker-boundaries.md` |
| Depends on | P077 closeout readiness, P084 executable rollout gates, P088 code-writer completion receipts, P092 retry authority recovery, P082 recovery/retry matrix, UI action boundary |
| Related | P095 two-phase agent invocation, `docs/reference/execution-truth-and-recovery.md`, `docs/reference/test-gates.md`, `docs/reference/rust-control-plane.md`, `docs/reference/ui-action-boundary.md`, implementation audit artifacts beside active proposals |
| Scope | Detect when a quality gate is blocked by work that cannot or should not be solved by another implementation loop, classify the remaining tail, and route the run through workflow-declared transitions. |
| Non-goal | No weakening of proposal gates, no automatic acceptance of incomplete code, no hidden waiver path, no replacement for implementation audits, and no human-selected ad hoc transitions. |

---

## 1. Correction from the previous draft

The previous P094 draft allowed the human approval screen to expose multiple semantic decisions such as:

- `accept_boundary`
- `reject_boundary`
- `request_followup_proposal`
- `return_to_implementation`

That is not the desired architecture.

This rewrite makes the boundary explicit:

> **All transitions are defined by workflow.**
> **Human approval only returns `accept` or `reject` with an optional/required comment.**

The human does not choose which workflow state comes next.

The workflow owns transitions such as:

- return to implementation;
- generate follow-up proposal seed;
- close current local implementation slice;
- block on external evidence;
- continue release readiness;
- stop as systemic blocker.

The approval only answers:

```text
Do you accept this blocker-boundary assessment?
```

with:

```text
accept / reject + comment
```

---

## 2. Problem

Implementation runs can burn large amounts of provider time after code-owned work is already mostly complete.

The run gets stuck in a quality gate that is hard or impossible to satisfy from the active worktree.

Typical examples:

- remote-only UI evidence requires a separate machine or operator action;
- dogfood evidence needs long-running live usage rather than another code patch;
- release/archive/push proof depends on human/environment state;
- provider quota/tooling conditions block a gate but do not imply more code should be written;
- audit asks for a broad matrix that is valid future work, not same-worktree work;
- proposal scope mixes independent slices, causing the current implementation worker to chase non-local blockers.

The bad behavior is not merely that a run becomes blocked.

The bad behavior is:

> the orchestrator keeps treating every blocker as if another code-writing loop could fix it.

That creates:

- repeated retries;
- stale review findings;
- large timelines;
- high provider burn;
- unclear operator decisions;
- and proposal loops that never converge.

---

## 3. Goal

P094 introduces a workflow-owned quality-gate boundary mechanism.

The system should be able to distinguish:

1. **Local code tail**
   Work that can still be completed inside the current worktree by code/docs/test agents.

2. **Follow-up code tail**
   Real code-owned work that is independent, large, or outside the approved current slice and should become a follow-up proposal.

3. **External blocker**
   Required proof/action that cannot be produced by the current Chainworks worker inside the current worktree.

4. **Invalid blocker claim**
   A claimed blocker that is actually solvable by code, tests, docs, normal MCP action, or existing workflow path.

The proposal must prevent blind repeated implementation loops while preserving strict quality gates.

---

## 4. Core principles

## 4.1 Workflow owns transitions

The workflow definition owns all transitions.

P094 may add new workflow tasks or states, but it must not let a lead agent or human approval dynamically choose the next state.

A blocker-boundary assessment produces data.
Workflow conditions consume that data.
The workflow routes accordingly.

## 4.2 Human approval is accept/reject only

Human approval payload:

```json
{
  "decision": "accept",
  "comment": "I accept that the remaining proof requires external dogfood evidence."
}
```

or:

```json
{
  "decision": "reject",
  "comment": "The claimed external blocker is invalid; the remaining issue is a local test failure."
}
```

No other approval actions are allowed.

In particular, the approval UI/API must not expose:

- `request_followup_proposal`
- `return_to_implementation`
- `close_current_slice`
- `create_followup`
- `advance_anyway`
- `waive_gate`

Those are workflow transitions or separate MCP/operator commands, not approval choices.

## 4.3 Lead/orchestrator is advisory, not authoritative

A lead agent may produce a candidate assessment.

The server-owned evaluator validates it.

The canonical boundary status is server-authored.

Lead output is evidence/input, not transition authority.

## 4.4 No weakening of gates

Accepting a blocker boundary does not mean all gates are green.

It means:

> the operator accepts the classification of the remaining tail.

If a release gate still requires external evidence, the run remains blocked/pending at the workflow-defined external-evidence state until that evidence exists.

---

## 5. Vocabulary

| Term | Meaning |
|---|---|
| `local_code_tail` | Code/test/doc work that can be completed inside the current worktree by a Chainworks implementation agent. |
| `followup_code_tail` | Code-owned work that is real but independent, large, or outside the approved current proposal slice. |
| `external_blocker` | Required proof/action that cannot be produced by the current worker in the current worktree. |
| `invalid_blocker_claim` | Claimed blocker that is actually solvable by local implementation, test, docs, or known MCP/workflow action. |
| `blocker_boundary` | Point where all locally solvable work is complete and remaining tail is follow-up code scope or external proof/action. |
| `split_candidate` | Proposal section or acceptance criterion that should become a separate proposal before implementation starts. |
| `blocker_signature_id` | Stable identity for a recurring blocker class + target + evidence scope. |
| `evidence_fingerprint` | Stable fingerprint of the evidence set used to classify a blocker. |

---

## 6. Proposed design

## 6.1 Pre-implementation decomposition

During proposal refinement, before implementation approval, the workflow may run a decomposition task.

This task reads:

- current proposal;
- proposal-review findings;
- acceptance criteria;
- rollout gates;
- required evidence locations;
- known remote-only or operator-only requirements;
- current reference docs for relevant subsystems.

It produces:

```text
proposal_decomposition_plan_v1
```

This artifact is advisory until consumed by explicit workflow conditions.

### Example

```json
{
  "schema_version": "proposal_decomposition_plan_v1",
  "proposal_id": "P094",
  "revision_id": "p094-r2",
  "implementation_slices": [
    {
      "slice_id": "boundary_evaluator",
      "summary": "Classify quality-gate blockers at implementation closeout.",
      "owned_by_current_proposal": true,
      "requires_new_proposal": false,
      "blocking_external_dependencies": []
    }
  ],
  "external_blockers": [
    {
      "blocker_id": "remote_ui_proof",
      "class": "remote_environment_required",
      "required_before_release": true,
      "chainworks_can_execute": false,
      "operator_action_required": true,
      "evidence_required": "remote UI proof log and screenshot bundle"
    }
  ],
  "split_candidates": [
    {
      "candidate_id": "expanded_reliability_matrix",
      "reason": "The reliability matrix exceeds the current proposal implementation slice.",
      "recommended_followup_title": "Reliability Matrix Expansion"
    }
  ],
  "implementation_start_decision": "ready_with_declared_boundaries"
}
```

### Workflow usage

The workflow decides how to route based on this artifact.

For example:

```yaml
state: proposal_decomposition_reviewed
transitions:
  - when: proposal_decomposition_plan.requires_split == true
    to: proposal_refinement
  - when: proposal_decomposition_plan.implementation_start_decision == "ready_with_declared_boundaries"
    to: implementation_approval
```

The artifact does not route itself.

---

## 6.2 Runtime blocker-boundary assessment

When implementation review or closeout readiness reports blockers, the workflow runs a blocker-boundary assessment before scheduling another code refinement.

Inputs:

- latest implementation audit report;
- `implementation_self_assessment_v2`;
- code-writer completion receipts;
- proposal gate output;
- closeout readiness output;
- side-effect ledger state;
- active artifact index;
- worktree fingerprint;
- changed-file manifest;
- current run/stage/agent execution state;
- pre-implementation decomposition plan if present.

It produces:

```text
quality_gate_blocker_assessment_v1
```

### Example

```json
{
  "schema_version": "quality_gate_blocker_assessment_v1",
  "run_id": "example",
  "stage_execution_id": "state_9_implementation_reviewed.3",
  "assessment_id": "qgba_...",
  "local_code_tail": [],
  "followup_code_tail": [
    {
      "blocker_signature_id": "followup:expanded_test_matrix:p081",
      "summary": "Implement additional reliability matrix rows not needed for the current slice.",
      "requires_new_proposal": true,
      "suggested_proposal_title": "Reliability Matrix Expansion for P081",
      "affected_surfaces": ["recovery matrix", "retry fixtures"],
      "acceptance_criteria_seed": [
        "Add additional rows for long-running soak and provider quota windows."
      ]
    }
  ],
  "external_blockers": [
    {
      "blocker_signature_id": "external:remote_ui_proof:p031",
      "class": "remote_environment_required",
      "why_not_chainworks_solvable": "The evidence must be produced on the approved remote macOS host with accessibility automation enabled.",
      "required_evidence": "remote gate log and accessibility screenshot bundle",
      "suggested_owner": "human_operator",
      "release_blocking": true
    }
  ],
  "invalid_or_weak_claims": [],
  "evidence_fingerprint": "sha256:...",
  "confidence": "high"
}
```

The assessment is never actionable by itself.

---

## 6.3 Server-owned validation

The server-owned `QualityGateBoundaryEvaluator` validates the candidate assessment.

It rejects the assessment if:

- any `local_code_tail` item is unresolved and marked non-blocking without evidence;
- an `external_blocker` lacks concrete `why_not_chainworks_solvable`;
- a blocker says “cannot be done” while required action maps to known MCP tool or local gate;
- follow-up code tail does not name affected surfaces and acceptance criteria;
- proposal gate is failing on ordinary compile/test/lint failures;
- side-effect ledger has unresolved effects;
- worktree has uncommitted generated changes that belong to the current proposal and are not committed or intentionally excluded;
- assessment conflicts with active implementation audit findings;
- assessment uses stale/superseded artifacts as proof of current local work;
- output freshness rules from P088 are not satisfied;
- blocker signature/evidence fingerprint repeats without measurable progress.

The evaluator produces:

```text
blocker_boundary_status_v1
```

### Example

```json
{
  "schema_version": "blocker_boundary_status_v1",
  "status": "awaiting_human_boundary_approval",
  "local_work_complete": true,
  "followup_proposal_required": true,
  "external_blocker_count": 1,
  "invalid_claim_count": 0,
  "hard_blockers": [
    {
      "blocker_signature_id": "external:remote_ui_proof:p031",
      "class": "remote_environment_required",
      "severity": "hard",
      "release_blocking": true,
      "evidence_fingerprint": "sha256:...",
      "allowed_workflow_routes": ["human_boundary_approval"],
      "forbidden_routes": ["implementation_refine", "release_complete"]
    }
  ],
  "workflow_route_hint": "human_boundary_approval"
}
```

`workflow_route_hint` is advisory and must match workflow-declared transitions.

---

## 7. Workflow-owned transitions

P094 requires workflows to declare transitions explicitly.

Example shape:

```yaml
states:
  implementation_reviewed:
    tasks:
      - evaluate_quality_gate_blocker_boundary
    transitions:
      - when: blocker_boundary_status.status == "local_code_tail_present"
        to: implementation_refine
      - when: blocker_boundary_status.status == "invalid_claim"
        to: implementation_refine
      - when: blocker_boundary_status.status == "awaiting_human_boundary_approval"
        to: blocker_boundary_approval
      - when: blocker_boundary_status.status == "pass"
        to: next_release_or_closeout_state

  blocker_boundary_approval:
    approval:
      kind: blocker_boundary
      allowed_decisions:
        - accept
        - reject
    transitions:
      - when: approval.decision == "accept"
        to: blocker_boundary_accepted
      - when: approval.decision == "reject"
        to: implementation_refine

  blocker_boundary_accepted:
    tasks:
      - emit_blocker_boundary_closeout
      - maybe_generate_followup_proposal_seed
    transitions:
      - when: blocker_boundary_status.has_release_blocking_external_blockers == true
        to: blocked_pending_external_evidence
      - when: blocker_boundary_status.followup_proposal_required == true
        to: close_current_slice_with_followup_seed
      - when: blocker_boundary_status.has_no_release_blocking_external_blockers == true
        to: next_release_or_closeout_state
```

Important:

- approval returns only `accept` or `reject`;
- follow-up proposal generation is a workflow task;
- returning to implementation is a workflow transition on rejection;
- external-evidence blocking is a workflow state;
- no approval payload selects arbitrary route.

---

## 8. Approval contract

## 8.1 `blocker_boundary_approval_request_v1`

```json
{
  "schema_version": "blocker_boundary_approval_request_v1",
  "approval_id": "...",
  "run_id": "...",
  "stage_execution_id": "...",
  "blocker_boundary_status_artifact_id": "...",
  "question": "Do you accept this blocker-boundary assessment?",
  "allowed_decisions": ["accept", "reject"],
  "summary": {
    "local_work_complete": true,
    "followup_proposal_required": true,
    "external_blocker_count": 1,
    "release_blocking_external_blocker_count": 1
  }
}
```

## 8.2 `blocker_boundary_human_decision_v1`

```json
{
  "schema_version": "blocker_boundary_human_decision_v1",
  "approval_id": "...",
  "decision": "accept",
  "comment": "I accept that remote UI proof must be collected outside the current implementation worktree.",
  "decided_at": "2026-05-26T00:00:00Z",
  "decided_by": "operator"
}
```

Allowed `decision` values:

- `accept`
- `reject`

`comment` is required on reject and optional on accept.

---

## 9. Human approval semantics

## 9.1 Accept

`accept` means:

> The operator accepts the boundary classification.

It does not mean:

- all gates are green;
- release evidence exists;
- run is successful;
- follow-up proposal is automatically approved;
- missing external evidence is waived.

After accept, workflow-declared transitions decide what happens next.

Possible workflow outcomes:

- emit closeout report;
- create follow-up proposal seed;
- block on external evidence;
- continue to release readiness if no release-blocking blockers remain.

## 9.2 Reject

`reject` means:

> The operator rejects the blocker-boundary classification.

The workflow routes back to implementation/refinement with the comment as durable operator feedback.

The next implementation instruction must include:

- rejection comment;
- rejected blocker signatures;
- required evidence or correction;
- instruction not to repeat the same unsupported blocker claim without new evidence.

---

## 10. Follow-up proposal seed

If workflow transitions require follow-up proposal generation, the system emits:

```text
followup_proposal_seed_v1
```

The seed includes:

- parent run id;
- parent proposal id/revision;
- exact unresolved code-owned scope;
- excluded external blockers;
- artifacts and audit findings that justify the split;
- proposed acceptance criteria;
- proof that the parent worktree has no remaining local code tail for the current slice.

The seed is not automatically merged into `docs/proposals/`.

It must enter the normal idea/proposal workflow or be explicitly accepted by an operator through the workflow-defined path.

---

## 11. No-progress rule

P094 must prevent repeated no-progress loops.

Rule:

```text
same blocker_signature_id
+ same evidence_fingerprint
+ no measurable local work progress
+ retry/refine budget exhausted
= no more implementation refinement for that blocker
```

The resulting status should be:

```text
blocked_no_progress
```

Allowed workflow routes:

- human boundary approval;
- collect new evidence;
- create follow-up proposal seed;
- systemic fix path if workflow declares one.

Forbidden workflow routes:

- blind retry;
- blind implementation refinement;
- release/closeout as green.

---

## 12. Classification model

## 12.1 External blocker classes

Initial vocabulary:

- `remote_environment_required`
- `human_operator_required`
- `provider_quota_or_capacity_wait`
- `credential_or_permission_required`
- `release_or_distribution_required`
- `live_dogfood_duration_required`
- `long_running_soak_required`
- `third_party_service_required`
- `hardware_or_device_required`
- `unavailable_in_current_worktree`

## 12.2 Follow-up code tail classes

Initial vocabulary:

- `independent_feature_slice`
- `expanded_test_matrix`
- `deferred_runtime_hardening`
- `separate_api_surface`
- `separate_ui_surface`
- `migration_or_backfill_slice`
- `performance_or_load_proof_slice`

## 12.3 Invalid claim classes

Initial vocabulary:

- `ordinary_test_failure`
- `compile_failure`
- `missing_local_gate_run`
- `known_mcp_tool_available`
- `local_file_edit_required`
- `docs_update_required`
- `proposal_scope_not_exhausted`
- `evidence_claim_without_artifact`
- `stale_artifact_claim`
- `output_freshness_failure`

---

## 13. API and readback

## 13.1 New artifacts

| Artifact | Purpose |
|---|---|
| `proposal_decomposition_plan_v1` | Pre-implementation slice/external dependency map. |
| `quality_gate_blocker_assessment_v1` | Candidate assessment from lead/orchestrator/assessment task. |
| `blocker_boundary_status_v1` | Server-validated canonical boundary status. |
| `blocker_boundary_approval_request_v1` | Human approval request with accept/reject only. |
| `blocker_boundary_human_decision_v1` | Durable human accept/reject decision with comment. |
| `followup_proposal_seed_v1` | Seed for follow-up proposal when workflow requires it. |

## 13.2 GraphQL readback

GraphQL may expose:

- `proposalDecompositionPlan`
- `qualityGateBlockerAssessment`
- `blockerBoundaryStatus`
- `blockerBoundaryHumanDecision`
- `followupProposalSeeds`

GraphQL mutations remain limited to approval decisions as defined by UI Action Boundary.

No additional GraphQL mutation is introduced by P094.

## 13.3 MCP readback

MCP `runs.get` and reports may expose:

- `proposal_decomposition_plan`
- `quality_gate_blocker_assessment`
- `blocker_boundary_status`
- `blocker_boundary_human_decision`
- `followup_proposal_seeds`

Any operational action must be an existing workflow/MCP command, not a special human approval option.

---

## 14. Rollout plan

## Phase 0 — Fixtures and historical runs

Collect historical implementation runs where audits looped on hard gates.

Fixture inventory:

- remote UI proof missing;
- release evidence missing;
- dogfood/soak evidence missing;
- ordinary code blocker;
- mixed code tail plus external blocker;
- invalid blocker explanation rejected by human;
- repeated same blocker with no progress;
- stale artifact claim.

## Phase 1 — Proposal decomposition artifacts

Add:

- `proposal_decomposition_plan_v1`;
- readback only;
- no workflow behavior change.

Make decomposition mandatory only for high-risk proposals at first.

High-risk signals:

- release/distribution evidence;
- remote UI proof;
- live dogfood/soak requirement;
- persistence/recovery/runtime changes;
- multiple independently releasable surfaces;
- migration/backfill;
- provider/runtime tooling.

## Phase 2 — Runtime blocker-boundary assessment

Add:

- `quality_gate_blocker_assessment_v1`;
- `QualityGateBoundaryEvaluator`;
- `blocker_boundary_status_v1`;
- readback via GraphQL/MCP/reports;
- no transition enforcement yet.

## Phase 3 — Workflow-declared approval boundary

Add workflow state for blocker-boundary approval.

Human approval supports only:

- `accept`
- `reject`

Transitions are declared in workflow.

## Phase 4 — Enforcement

Replace unbounded code-refine loops with workflow-declared decision routing.

Enforce:

- repeated identical blocker claims need new evidence or human acceptance;
- local code tail routes back to implementation;
- invalid claims route back to implementation;
- external/follow-up boundary goes to approval;
- accepted boundary follows workflow-defined closeout/follow-up/external-evidence path.

---

## 15. Metrics

Required metrics:

- `quality_gate_blocker_assessments_total{status,class}`
- `quality_gate_blocker_validation_rejections_total{reason}`
- `implementation_refine_loops_avoided_total{proposal_id}`
- `followup_proposal_seeds_created_total{tail_class}`
- `external_blockers_accepted_total{blocker_class}`
- `invalid_blocker_claims_total{claim_class}`
- `human_boundary_approval_latency_seconds`
- `post_boundary_reopen_total{reason}`
- `false_external_blocker_rate`
- `repeated_blocker_no_progress_total{signature}`

Guardrail metric:

- `accepted_boundary_later_rejected_percent`

The feature is successful only if repeated implementation-refine loops decrease without increasing false closeouts.

---

## 16. Test plan

Add retained gate:

```bash
./scripts/test-gate.sh proposal-094
./scripts/test-gate.sh p094
```

The gate must prove:

1. proposal decomposition detects split candidates before implementation;
2. local compile/test failures cannot be classified as external blockers;
3. remote UI evidence can be classified as external only when required proof is named;
4. mixed local code tail plus external blocker routes back to implementation until local tail is empty;
5. pure external blocker tail routes to human boundary approval;
6. human approval exposes only accept/reject;
7. human rejection returns to implementation through workflow-defined transition;
8. follow-up proposal seed is generated only through workflow-defined task;
9. GraphQL/MCP/report readback agree;
10. macOS readback is passive except approval accept/reject;
11. repeated identical blocker claims without new evidence fail closed;
12. lead recommendation cannot override server validation;
13. accepted boundary does not mark release-blocking external evidence as satisfied.

---

## Relationship to P095: Two-Phase Agent Invocation

P095 adds an intermediate classification before blocker-boundary assessment for
implementation stages that performed work but did not settle output.

Missing output after a work turn should route to P095 output collection and, if
that fails, P079 repair/fallback before P094 classifies the remaining condition
as a blocker boundary. P094 assessment must distinguish:

- work not done;
- work done but output not collected;
- output collected but the quality gate still blocked.

Workflow transitions remain workflow-owned. Human approval remains accept/reject
only and must not become an ad hoc choice between output repair, implementation
retry, and blocker-boundary acceptance.

---

## 17. Acceptance criteria

P094 is implemented only when:

- workflows declare all blocker-boundary transitions explicitly;
- human approval accepts only `accept` or `reject` with comment;
- proposal decomposition can identify split candidates before implementation starts;
- implementation closeout can distinguish local code tail, follow-up code tail, external blockers, and invalid blocker claims;
- server-owned validation rejects weak blocker claims;
- repeated no-progress blocker loops are stopped;
- rejection returns to implementation with durable feedback;
- follow-up proposal seeds are generated only by workflow-declared tasks;
- external-only blockers do not generate unnecessary new proposals;
- accepting a boundary does not mark release-blocking evidence as satisfied;
- all readback surfaces expose the same boundary status;
- retained `proposal-094` gate proves positive and negative cases.

---

## 18. Open questions

1. Should `proposal_decomposition_plan_v1` be mandatory for all proposals or only proposals above a risk/complexity threshold?
2. Should follow-up proposal seeds create Ideas automatically through workflow, or remain files until a separate operator/MCP action accepts them?
3. Which approval owner can accept a boundary: any operator, release owner, or proposal owner?
4. Should provider quota waits be external blockers or normal retry scheduling unless they exceed a wall-clock threshold?
5. What is the maximum number of repeated same-signature blocker observations before `blocked_no_progress` is mandatory?

---

## 19. Final recommendation

P094 should not make the lead smarter.

It should make the workflow stricter.

The key rule is:

> facts produce a server-validated blocker boundary;
> workflow decides transitions;
> human approval only accepts or rejects the boundary explanation.

This preserves quality gates while preventing endless implementation loops against blockers that the current worktree cannot solve.
