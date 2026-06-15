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
| Non-goal | No code fixes, no weakening of proposal gates, no automatic acceptance of incomplete code, no hidden waiver path, no replacement for implementation audits, no replacement for output contract settlement or side-effect reconciliation, no new UI actions, no arbitrary human approval actions, no generic rule engine, and no human-selected ad hoc transitions. |

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

## 4.5 Lower-layer settlement preconditions

P094 may only evaluate quality-gate blocker boundaries after lower-layer execution truth is settled enough to evaluate.

P094 is not the first responder for broken execution settlement. If a required producer invocation is unsettled, if active contract truth is missing, or if a side-effect ledger entry is unresolved, the workflow must route to the owning settlement/recovery path before quality-gate blocker-boundary evaluation.

P094 must not classify these conditions as quality-gate blocker boundaries:

- required output missing for a required producer;
- agent invocation not settled;
- stage marked completed without required output settlement;
- active contract row missing;
- output freshness failure;
- interrupted provider turn where meaningful work may have happened but output settlement did not complete;
- side-effect ledger unresolved;
- stale or superseded artifact being used as current proof.

Those conditions route to the owning subsystem first:

- output contract settlement / repair;
- agent invocation settlement;
- side-effect reconciliation;
- retry authority recovery;
- output freshness / P088;
- provider-session resurrection or output-only recovery where applicable.

Accepting a blocker boundary must not mask missing release evidence, missing output settlement, failed gates, or unresolved side effects.

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
| `evidence_freshness` | Freshness of the evidence set relative to the latest relevant implementation pass: `fresh`, `stale`, `unknown`, or `superseded`. |
| `owner_class` | Workflow owner class that determines which route may handle a blocker. |
| `is_code_writer_blocking` | Boolean showing whether this blocker may return to implementation refinement. |

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
- security/prepush/docs/review artifacts with generation metadata;
- `implementation_self_assessment_v2`;
- `tests_result_v1`;
- code-writer completion receipts;
- proposal gate output;
- closeout readiness output;
- side-effect ledger state;
- active artifact index;
- active artifact contract rows and output settlement status;
- worktree fingerprint;
- changed-file manifest;
- current run/stage/agent execution state;
- latest relevant code-writer stage/agent execution ids;
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
  "blockers": [
    {
      "blocker_signature_id": "security:graphql-live-auth:p082",
      "evidence_fingerprint": "sha256:security-report-generation-7",
      "evidence_freshness": "fresh",
      "source_artifact_generation_id": "artifact_generation.security_report.7",
      "observed_after_stage_execution_id": "state_10_implementation_refined.6",
      "observed_after_agent_execution_id": "agent_execution.code_writer.6",
      "owner_class": "code_writer",
      "is_code_writer_blocking": true,
      "gate_command": "cargo test -p graphql-server live_principal_reload",
      "evidence_refs": [
        "security/report.json#SEC-HIGH-001",
        "control-plane/crates/graphql-server/src/server.rs:445"
      ]
    }
  ],
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

Every blocker item must carry:

- `blocker_signature_id`;
- `evidence_fingerprint`;
- `evidence_freshness`;
- `source_artifact_generation_id` or equivalent active contract generation pointer when available;
- `observed_after_stage_execution_id`;
- `observed_after_agent_execution_id`;
- `owner_class`;
- `is_code_writer_blocking`;
- `gate_command` when applicable;
- `evidence_refs`.

`evidence_freshness` values:

- `fresh`;
- `stale`;
- `unknown`;
- `superseded`.

Freshness rules:

- stale or superseded blockers cannot keep the implementation loop alive;
- unknown freshness fails closed into review refresh / evidence refresh, not code refinement;
- fresh blocker evidence can reset no-progress counters only if it changes the `blocker_signature_id` or `evidence_fingerprint`;
- stale review artifacts are not active blockers even when their status is `block` or `needs_code_fixes`.

The assessment is never actionable by itself.

---

## 6.3 Server-owned validation

The server-owned `QualityGateBoundaryEvaluator` validates the candidate assessment.

It rejects the assessment if:

- lower-layer settlement preconditions are not satisfied;
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

It must produce one of the lower-layer statuses instead of boundary approval when the owning subsystem has not settled:

- `output_settlement_required`;
- `side_effect_reconciliation_required`;
- `runtime_recovery_required`;
- `review_refresh_required`.

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
  "blockers": [
    {
      "blocker_signature_id": "external:remote_ui_proof:p031",
      "evidence_fingerprint": "sha256:...",
      "evidence_freshness": "fresh",
      "source_artifact_generation_id": "artifact_generation.ui_review.4",
      "observed_after_stage_execution_id": "state_9_implementation_reviewed.3",
      "observed_after_agent_execution_id": "agent_execution.ui_reviewer.3",
      "owner_class": "release_evidence",
      "is_code_writer_blocking": false,
      "gate_command": "./scripts/test-gate.sh ui-smoke",
      "evidence_refs": ["docs/evidence/ui-smoke/latest.json"]
    }
  ],
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

## 6.4 Owner-classified blocker model

Every blocker must be classified with exactly one `owner_class`:

- `code_writer`;
- `docs_guardian`;
- `security_reviewer`;
- `prepush_reviewer`;
- `implementation_auditor`;
- `operator`;
- `release_evidence`;
- `external_environment`;
- `followup_proposal`;
- `runtime_recovery`;
- `output_settlement`;
- `side_effect_reconciliation`;
- `review_refresh`;
- `unknown`.

Routing rules:

- only `owner_class = code_writer` with `is_code_writer_blocking = true` may route back to implementation refinement;
- `docs_guardian` routes to the docs workflow;
- `security_reviewer`, `prepush_reviewer`, and `implementation_auditor` normally route to fresh review/evidence refresh unless they cite concrete source/test defects;
- `release_evidence`, `external_environment`, and `operator` route to handoff / external evidence states;
- `followup_proposal` routes to workflow-defined follow-up proposal seed generation;
- `output_settlement`, `side_effect_reconciliation`, and `runtime_recovery` route to their owning lower-layer recovery paths, not P094 boundary approval;
- `unknown` fails closed and must not schedule `code_writer` without file-level evidence.

If a reviewer wants to send work back to `code_writer`, the finding must include concrete source/test/doc file-level evidence. A vague gate, evidence, doc, or review-refresh finding must not become `needs_code_fixes`.

---

## 6.5 Review artifact freshness

Security, prepush, audit, docs, and implementation-summary artifacts are active blockers only when they are fresh relative to the latest relevant implementation pass.

If security/prepush/audit/implementation-summary artifacts are older than the latest relevant implementation/code-writer pass, they cannot be used as active blockers. The evaluator must emit:

```text
review_refresh_required
```

not:

```text
implementation_refine
```

Example:

```text
security/report.json is block, but artifact generation predates the latest code_writer execution that addressed security findings. Workflow must request fresh security review before returning to code_writer.
```

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
      - when: blocker_boundary_status.status == "output_settlement_required"
        to: output_settlement_recovery
      - when: blocker_boundary_status.status == "side_effect_reconciliation_required"
        to: side_effect_reconciliation
      - when: blocker_boundary_status.status == "runtime_recovery_required"
        to: runtime_recovery
      - when: blocker_boundary_status.status == "review_refresh_required"
        to: implementation_review_refresh
      - when: blocker_boundary_status.status == "local_code_tail_present"
        to: implementation_refine
      - when: blocker_boundary_status.status == "invalid_claim"
        to: implementation_review_refresh
      - when: blocker_boundary_status.status == "blocked_no_progress"
        to: blocker_boundary_approval
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
        to: implementation_review_refresh

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
- returning to implementation requires a workflow-declared route and fresh code-owned evidence;
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

Complement rule:

```text
new blocker_signature_id
or changed evidence_fingerprint
or fresh source/test evidence
= may allow targeted implementation/review action if owner_class permits it
```

This prevents stale loops without blocking genuinely new security, prepush, audit, or code defects.

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

Readback surfaces must expose, for every blocker:

- `blocker_signature_id`;
- `evidence_fingerprint`;
- `evidence_freshness`;
- `source_artifact_generation_id` or equivalent active contract generation pointer;
- `observed_after_stage_execution_id`;
- `observed_after_agent_execution_id`;
- `owner_class`;
- `is_code_writer_blocking`;
- `gate_command`;
- `evidence_refs`;
- allowed workflow route.

---

## 14. Reviewer and auditor blocker schema guidance

Reviewer and auditor outputs should include structured blocker records that P094 can validate without relying on prose.

Example:

```json
{
  "summary": "GraphQL HTTP auth still reads a startup PrincipalTable.",
  "owner_class": "code_writer",
  "is_code_writer_blocking": true,
  "freshness": "fresh",
  "evidence_refs": ["security/report.json#SEC-HIGH-001"],
  "gate_command": "cargo test -p graphql-server live_principal_reload",
  "observed_after_stage_execution_id": "state_10_implementation_refined.6",
  "observed_after_agent_execution_id": "agent_execution.code_writer.6",
  "file_level_evidence": [
    "control-plane/crates/graphql-server/src/server.rs:445",
    "control-plane/crates/graphql-server/src/auth_layer.rs:45"
  ]
}
```

Strict guidance:

- if a reviewer wants work to return to `code_writer`, it must provide concrete source/test/doc file-level evidence;
- a vague gate/evidence/doc/review-refresh finding must not become `needs_code_fixes`;
- reviewer outputs without owner, freshness, and evidence fail closed to `unknown` or `review_refresh_required`;
- `security_reviewer`, `prepush_reviewer`, and `implementation_auditor` findings route to implementation only when the owner is explicitly code-owned and evidence is fresh.

---

## 15. Do not mask implementation settlement failures

P094 must not say "blocker boundary accepted" when the real issue is an execution/output settlement problem.

These are not quality-gate blocker boundaries:

- `implementation_self_assessment_v2` missing;
- `tests_result_v1` missing;
- `implementation_review_summary_v1` stale;
- `AgentOutputSettlement = missing_required_outputs`;
- active artifact contract row absent;
- stage completed without required output settlement;
- provider turn interrupted after meaningful progress but output collection did not settle.

Those must be resolved before P094 boundary evaluation. They route to output settlement, agent invocation settlement, provider-session recovery, retry authority recovery, or review refresh as appropriate.

---

## 16. Rollout plan

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
- stale artifact claim;
- stale security/prepush/audit block that predates latest code_writer work;
- missing required output or active contract row;
- side-effect ledger unresolved.

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
- local code tail routes back to implementation only when `owner_class = code_writer`, `is_code_writer_blocking = true`, and evidence is fresh;
- invalid or weak claims route to review/evidence refresh unless they contain fresh file-level source/test evidence;
- external/follow-up boundary goes to approval;
- lower-layer settlement failures route to their owning recovery path;
- accepted boundary follows workflow-defined closeout/follow-up/external-evidence path.

---

## 17. Metrics

Required metrics:

- `quality_gate_blocker_assessments_total{status,class}`
- `quality_gate_blocker_validation_rejections_total{reason}`
- `quality_gate_blocker_freshness_total{freshness,owner_class}`
- `implementation_refine_loops_avoided_total{proposal_id}`
- `followup_proposal_seeds_created_total{tail_class}`
- `external_blockers_accepted_total{blocker_class}`
- `invalid_blocker_claims_total{claim_class}`
- `review_refresh_required_total{artifact_kind}`
- `output_settlement_required_before_boundary_total{reason}`
- `human_boundary_approval_latency_seconds`
- `post_boundary_reopen_total{reason}`
- `false_external_blocker_rate`
- `repeated_blocker_no_progress_total{signature}`

Guardrail metric:

- `accepted_boundary_later_rejected_percent`

The feature is successful only if repeated implementation-refine loops decrease without increasing false closeouts.

---

## 18. Test plan

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
13. accepted boundary does not mark release-blocking external evidence as satisfied;
14. stale security/prepush/audit block artifacts emit `review_refresh_required` and do not route to `code_writer`;
15. fresh security/prepush blockers with file-level evidence can route to targeted implementation when `owner_class = code_writer`;
16. missing required outputs, missing `implementation_self_assessment_v2`, or missing active contract rows route to output settlement recovery and never boundary approval;
17. reviewer findings without owner/freshness/evidence fail closed to `unknown` or `review_refresh_required`;
18. release-evidence-only tails route to boundary/external evidence states, not implementation refinement.

Required fixture cases:

| Case | Fixture | Expected result |
|---|---|---|
| A. stale security block | `security/report.json` is `block` but predates the latest implementation pass. | `review_refresh_required`; no `code_writer` route. |
| B. fresh security block | security report is fresh and cites source file evidence. | local code tail route allowed when owner permits. |
| C. missing required output | `implementation_self_assessment_v2` missing. | output settlement recovery; no blocker-boundary approval. |
| D. repeated no-progress blocker | same blocker signature and evidence fingerprint over repeated cycles with no local progress. | `blocked_no_progress`. |
| E. handoff-only tail | implementation complete; no blocking code tasks; only release evidence remains. | boundary/external evidence route. |
| F. invalid weak blocker | reviewer says "needs code fixes" without file-level evidence. | `unknown` or `review_refresh_required`; no `code_writer` route. |
| G. human approval shape | approval request is generated. | allowed decisions are only `accept` and `reject`. |

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

## 19. Acceptance criteria

P094 is implemented only when:

- workflows declare all blocker-boundary transitions explicitly;
- human approval accepts only `accept` or `reject` with comment;
- proposal decomposition can identify split candidates before implementation starts;
- implementation closeout can distinguish local code tail, follow-up code tail, external blockers, and invalid blocker claims;
- server-owned validation rejects weak blocker claims;
- P094 does not run boundary approval when required output settlement is missing;
- P094 does not use stale review artifacts as current blockers;
- fresh prepush/security blockers with file-level evidence can route to targeted implementation when `owner_class` permits it;
- repeated no-progress blocker loops are stopped;
- repeated same blocker signature with the same evidence fingerprint and no local progress routes to `blocked_no_progress`;
- reviewer findings without owner/freshness/evidence fail closed and do not schedule `code_writer`;
- `review_refresh_required` is emitted when review artifacts predate the latest implementation pass;
- rejection returns through workflow-declared routing with durable feedback and may schedule `code_writer` only when fresh code-owned evidence exists;
- follow-up proposal seeds are generated only by workflow-declared tasks;
- external-only blockers do not generate unnecessary new proposals;
- accepting a boundary does not mark release-blocking evidence as satisfied;
- lead recommendation cannot override the server-owned evaluator;
- GraphQL/MCP/readback surfaces expose blocker freshness, `owner_class`, and allowed workflow route;
- all readback surfaces expose the same boundary status;
- retained `proposal-094` gate proves positive and negative cases.

---

## 20. Open questions

1. Should `proposal_decomposition_plan_v1` be mandatory for all proposals or only proposals above a risk/complexity threshold?
2. Should follow-up proposal seeds create Ideas automatically through workflow, or remain files until a separate operator/MCP action accepts them?
3. Which approval owner can accept a boundary: any operator, release owner, or proposal owner?
4. Should provider quota waits be external blockers or normal retry scheduling unless they exceed a wall-clock threshold?
5. What is the maximum number of repeated same-signature blocker observations before `blocked_no_progress` is mandatory?

---

## 21. Non-goals recap

P094 does not:

- implement code fixes;
- weaken gates;
- replace implementation audit;
- replace output contract settlement;
- replace side-effect reconciliation;
- add UI actions;
- add arbitrary human approval actions;
- create a generic rule engine;
- make lead/orchestrator authoritative over transitions.

---

## 22. Final recommendation

P094 should not make the lead smarter.

It should make the workflow stricter.

The key rule is:

> facts produce a server-validated blocker boundary;
> workflow decides transitions;
> human approval only accepts or rejects the boundary explanation.

This preserves quality gates while preventing endless implementation loops against blockers that the current worktree cannot solve.
