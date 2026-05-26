# Proposal 094: Orchestrated Quality-Gate Blocker Boundaries and Proposal Splitting

| Field | Value |
|---|---|
| Date | 2026-05-26 |
| Status | Draft |
| Author | Codex |
| Depends on | P077 closeout readiness, P084 executable rollout gates, P088 code-writer completion receipts, P092 retry authority recovery, current implementation-review workflow |
| Related | `docs/reference/execution-truth-and-recovery.md`, `docs/reference/test-gates.md`, `docs/reference/rust-control-plane.md`, implementation audit artifacts beside active proposals |
| Scope | Detect quality-gate blockers that cannot be resolved inside the current Chainworks implementation worktree, split work before implementation when possible, and produce auditable handoff/blocker artifacts when runtime implementation reaches a true external boundary. |
| Non-goal | No weakening of proposal gates, no automatic acceptance of incomplete code, no bypass of human approval, no hidden waiver path, and no replacement for proposal-review or implementation-audit quality standards. |

---

## 1. Problem

Current implementation runs often burn large amounts of provider time after the code-owned work is already mostly complete.
The run gets stuck in an implementation quality gate that is hard or impossible to satisfy from the active worktree.

Typical examples:

- remote-only UI evidence requires a separate machine or operator action;
- dogfood evidence needs long-running live usage rather than code changes;
- release/archive/push proof depends on human or environment state;
- quota/provider/tooling conditions block a gate but do not imply more code should be written;
- audit asks for a broad matrix that is valid as future work, not as a same-worktree fix;
- proposal scope mixes several independently releasable slices, causing the current implementation worker to chase non-local blockers.

The bad behavior is not merely that a run becomes blocked.
The bad behavior is that the orchestrator keeps treating every blocker as if another code-writing loop could fix it.
That creates repeated retries, stale review findings, large timelines, and unclear operator decisions.

We need a first-class boundary between:

1. code-owned implementation work that should continue in the current run;
2. code-owned remaining work that is too large or independent and should become a follow-up proposal;
3. non-code/external blockers that need evidence, operator action, release work, remote hardware, or explicit human acceptance;
4. invalid blocker explanations that should be rejected by the human approver and routed back to implementation.

## 2. Goal

P094 introduces a two-layer contract:

1. **Pre-implementation proposal decomposition.**
   During proposal refinement, the system must identify implementation slices, external proof dependencies, and gate conditions before work begins.
   A proposal may be split into multiple smaller proposals when the original scope contains independent code slices or known external blockers.

2. **Runtime blocker-boundary closeout.**
   During implementation, if a quality gate cannot be completed in the current worktree, the orchestrator must close all locally solvable work, classify the remaining tail, and produce durable artifacts.
   The run may then ask for human closeout approval with explicit blocker evidence.
   The human can accept the boundary, reject it, or send the run back for more work.

The design deliberately combines both approaches.
Pre-splitting reduces the chance of wasted implementation loops.
Runtime blocker-boundary detection remains necessary because real blockers are often only visible after code and evidence exist.

## 3. Non-Goals

- Do not weaken `proposal-XXX` gates or allow a green status without evidence.
- Do not let agents invent excuses for unfinished implementation.
- Do not mark a run complete only because the code writer claims there is no code left.
- Do not auto-create follow-up proposals without operator-visible justification.
- Do not replace implementation audits.
- Do not classify ordinary compile/test failures as external blockers.
- Do not use exported JSON files as transition authority when SQLite/runtime truth exists.
- Do not make remote UI/dogfood/release evidence optional; this proposal only makes unresolved proof boundaries explicit and governed.

## 4. Vocabulary

| Term | Meaning |
|---|---|
| `local_code_tail` | Code/test/doc work that can be completed inside the current worktree by a code/documentation agent. |
| `followup_code_tail` | Code-owned work that is real but independent, large, or outside the approved current proposal slice. It should become a follow-up proposal rather than extend the current run indefinitely. |
| `external_blocker` | Required proof or action that cannot be produced by the current Chainworks worker in the current worktree. Examples: remote-only UI proof, live dogfood duration, human release action, unavailable hardware, provider quota wait, operator credentials. |
| `invalid_blocker_claim` | A claimed blocker that is actually solvable by code changes, tests, docs, or normal MCP actions in the current run. |
| `blocker_boundary` | The point where all locally solvable work is complete and the remaining tail is either follow-up code scope or external proof/action. |
| `split_candidate` | A proposal section or acceptance criterion that should become a separate proposal before implementation starts. |

## 5. Proposed Design

### 5.1 Pre-implementation decomposition

During proposal refinement, before implementation approval, the orchestrator adds an explicit decomposition pass.

The pass reads:

- the current proposal;
- proposal-review findings;
- acceptance criteria and rollout gates;
- required evidence locations;
- known remote-only or operator-only requirements;
- current reference docs for relevant subsystems.

It produces `proposal_decomposition_plan_v1`:

```json
{
  "schema_version": "proposal_decomposition_plan_v1",
  "proposal_id": "P094",
  "revision_id": "p094-r1",
  "implementation_slices": [
    {
      "slice_id": "core_orchestrator_contract",
      "summary": "Detect and classify quality-gate blockers at implementation closeout.",
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
      "candidate_id": "large_followup_matrix",
      "reason": "The acceptance matrix exceeds the current proposal's implementation slice.",
      "recommended_action": "create_followup_proposal"
    }
  ],
  "implementation_start_decision": "ready_with_declared_boundaries"
}
```

The implementation approval stage should show this plan.
If the plan identifies large independent code scope, the proposal should be split before implementation begins.
If the remaining blockers are purely external proof dependencies, implementation may start, but the blockers are known and cannot surprise closeout.

### 5.2 Runtime blocker-boundary assessment

When an implementation review or closeout readiness gate reports blockers, the orchestrator runs a blocker-boundary assessment before scheduling another code refinement.

Inputs:

- latest implementation audit report;
- `implementation_self_assessment_v2`;
- proposal gate output;
- `implementation_closeout_readiness_v1`;
- side-effect readback;
- active artifact index;
- worktree fingerprint and changed-file manifest;
- current run/stage/agent execution state;
- pre-implementation decomposition plan if present.

Output: `quality_gate_blocker_assessment_v1`.

```json
{
  "schema_version": "quality_gate_blocker_assessment_v1",
  "run_id": "example",
  "stage_id": "state_9_implementation_reviewed",
  "assessment_id": "qgba_...",
  "local_code_tail": [],
  "followup_code_tail": [
    {
      "summary": "Implement remaining reliability matrix not needed for current slice.",
      "requires_new_proposal": true,
      "suggested_proposal_title": "Reliability Matrix Expansion for P081"
    }
  ],
  "external_blockers": [
    {
      "blocker_id": "remote_ui_proof_missing",
      "class": "remote_environment_required",
      "why_not_chainworks_solvable": "UI proof must run on the approved remote macOS host with automation enabled.",
      "required_evidence": "remote gate log and accessibility screenshot bundle",
      "suggested_owner": "human_operator",
      "release_blocking": true
    }
  ],
  "invalid_or_weak_claims": [],
  "recommended_decision": "request_human_boundary_approval",
  "confidence": "high"
}
```

The assessment is not trusted because an agent wrote it.
It becomes actionable only after deterministic validation.

### 5.3 Validation rules

The orchestrator must reject a blocker-boundary assessment if:

- any `local_code_tail` item is unresolved and marked non-blocking without evidence;
- an `external_blocker` lacks a concrete `why_not_chainworks_solvable`;
- a blocker says "cannot be done" while the required action maps to a known MCP tool or local gate;
- a follow-up code tail does not name affected surfaces and acceptance criteria;
- the proposal gate is failing on ordinary compile/test/lint failures;
- side effects are unresolved;
- the worktree has uncommitted generated changes that belong to the current proposal and are not committed or intentionally excluded;
- the assessment conflicts with active implementation audit findings.

Validated assessments produce `blocker_boundary_status_v1`:

```json
{
  "schema_version": "blocker_boundary_status_v1",
  "status": "awaiting_human_boundary_approval",
  "local_work_complete": true,
  "followup_proposal_required": true,
  "external_blocker_count": 1,
  "invalid_claim_count": 0,
  "human_decisions": ["accept_boundary", "reject_boundary", "request_followup_proposal", "return_to_implementation"]
}
```

### 5.4 Human approval loop

At the manual approval boundary, the operator sees:

- what is complete;
- what cannot be completed inside Chainworks;
- why the blocker is not solvable by another code loop;
- what evidence is missing;
- whether a follow-up proposal will be created;
- what happens if the operator accepts or rejects the explanation.

The human can choose:

| Decision | Result |
|---|---|
| `accept_boundary` | Current run can close with explicit blocker/handoff artifacts if no local code tail remains. |
| `reject_boundary` | Assessment is marked rejected; run returns to implementation with the rejection reason as operator instruction. |
| `request_followup_proposal` | Orchestrator creates a new proposal seed from `followup_code_tail`. |
| `return_to_implementation` | No closeout; code/documentation agents continue in the current run. |

Human rejection must be durable.
The next code/refine stage receives the rejection reason and must not repeat the same unsupported blocker claim without new evidence.

### 5.5 Follow-up proposal generation

If `followup_code_tail` is non-empty, the orchestrator generates `followup_proposal_seed_v1`.

The seed includes:

- parent run id;
- parent proposal id/revision;
- exact unresolved code-owned scope;
- excluded external blockers;
- artifacts and audit findings that justify the split;
- proposed acceptance criteria;
- proof that the parent worktree has no remaining local code tail for the current slice.

The seed is not automatically merged into `docs/proposals/`.
It must enter the normal idea/proposal workflow or be explicitly accepted by an operator.

### 5.6 Runtime behavior

The implementation closeout state changes from a simple loop:

```text
audit blocker -> code refinement -> audit blocker -> code refinement
```

to a bounded decision tree:

```text
audit blocker
  -> classify tail
  -> local code tail exists: refine current run
  -> followup code tail exists: create follow-up proposal seed, ask human
  -> only external blockers remain: ask human boundary approval
  -> invalid explanation: return to implementation with rejection reason
```

This keeps strict gates intact while preventing empty loops against known external constraints.

## 6. API and Artifact Contracts

### 6.1 New artifacts

| Artifact | Path | Owner |
|---|---|---|
| `proposal_decomposition_plan_v1` | `reviews/proposal/decomposition-plan.json` | proposal refinement/review stage |
| `quality_gate_blocker_assessment_v1` | `review/quality-gate-blocker-assessment.json` | implementation closeout assessment |
| `blocker_boundary_status_v1` | `review/blocker-boundary-status.json` | orchestrator validation |
| `followup_proposal_seed_v1` | `followups/proposal-seed.json` | orchestrator after validated follow-up tail |
| `blocker_boundary_human_decision_v1` | `review/blocker-boundary-human-decision.json` | approval resolution |

### 6.2 MCP / GraphQL readback

Add read-only fields to run readback:

- `proposalDecompositionPlan`
- `qualityGateBlockerAssessment`
- `blockerBoundaryStatus`
- `followupProposalSeeds`
- `blockerBoundaryHumanDecisions`

MCP `runs.get` and reports must expose the same state using snake_case fields:

- `proposal_decomposition_plan`
- `quality_gate_blocker_assessment`
- `blocker_boundary_status`
- `followup_proposal_seeds`
- `blocker_boundary_human_decisions`

The macOS app may display this readback, but it does not decide the policy.
Policy decisions live in the control plane and approval tools.

## 7. Classification Model

### 7.1 External blocker classes

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

### 7.2 Follow-up code tail classes

Initial vocabulary:

- `independent_feature_slice`
- `expanded_test_matrix`
- `deferred_runtime_hardening`
- `separate_api_surface`
- `separate_ui_surface`
- `migration_or_backfill_slice`
- `performance_or_load_proof_slice`

### 7.3 Invalid claim classes

Initial vocabulary:

- `ordinary_test_failure`
- `compile_failure`
- `missing_local_gate_run`
- `known_mcp_tool_available`
- `local_file_edit_required`
- `docs_update_required`
- `proposal_scope_not_exhausted`
- `evidence_claim_without_artifact`

## 8. Rollout Plan

### Phase 0: Research and fixtures

- Collect 5-10 historical implementation runs where audits looped on hard gates.
- Build fixture inventory for:
  - remote UI proof missing;
  - release evidence missing;
  - dogfood/soak evidence missing;
  - ordinary code blocker;
  - mixed code tail plus external blocker;
  - invalid blocker explanation rejected by human.

### Phase 1: Proposal decomposition artifacts

- Add `proposal_decomposition_plan_v1`.
- Add proposal-readiness gate checks requiring explicit implementation slices and external blockers for high-risk proposals.
- Keep this advisory at first.

### Phase 2: Runtime blocker-boundary assessment

- Add `quality_gate_blocker_assessment_v1`.
- Validate it against audit findings and run-state truth.
- Expose readback via GraphQL/MCP/reports.
- Do not change transition behavior yet.

### Phase 3: Human approval loop

- Add governed approval decisions for blocker boundaries.
- Add rejection feedback loop into the next implementation instruction.
- Add follow-up proposal seed generation.

### Phase 4: Enforcement

- Replace unbounded code-refine loops with bounded decision routing.
- Enforce that repeated identical external blocker claims need new evidence or human acceptance.
- Add metrics and closeout readiness integration.

## 9. Metrics

Required metrics:

- `quality_gate_blocker_assessments_total{decision,class}`
- `quality_gate_blocker_rejections_total{reason}`
- `implementation_refine_loops_avoided_total{proposal_id}`
- `followup_proposal_seeds_created_total{tail_class}`
- `external_blockers_accepted_total{blocker_class}`
- `invalid_blocker_claims_total{claim_class}`
- `human_boundary_approval_latency_seconds`
- `post_boundary_reopen_total{reason}`
- `false_external_blocker_rate`

Guardrail metric:

- `accepted_boundary_later_rejected_percent`

The feature is successful only if repeated implementation-refine loops decrease without increasing false closeouts.

## 10. Test Plan

Add retained gate:

```bash
./scripts/test-gate.sh proposal-094
./scripts/test-gate.sh p094
```

The gate must prove:

1. proposal decomposition detects split candidates before implementation;
2. local compile/test failures cannot be classified as external blockers;
3. remote UI evidence can be classified as external only when the required remote proof is named;
4. mixed local code tail plus external blocker routes back to implementation until local tail is empty;
5. pure external blocker tail routes to human boundary approval;
6. human rejection returns to implementation with the rejection reason;
7. follow-up proposal seed includes exact parent evidence and excludes external blockers;
8. GraphQL/MCP/report readback agree;
9. macOS readback is passive and does not own policy;
10. repeated identical blocker claims without new evidence fail closed.

## 11. Acceptance Criteria

P094 is implemented only when:

- A proposal can declare decomposition/split boundaries before implementation starts.
- Implementation closeout can distinguish local code tail, follow-up code tail, external blockers, and invalid blocker claims.
- The orchestrator stops blind repeated code-refine loops when no local code tail remains.
- Human approval can accept or reject the boundary explanation.
- Rejection returns to implementation with durable feedback.
- Follow-up proposal seeds are generated only for real code-owned tail.
- External-only blockers do not generate unnecessary new proposals.
- All readback surfaces expose the same boundary status.
- The retained `proposal-094` gate proves positive and negative cases.

## 12. Open Questions

1. Should `proposal_decomposition_plan_v1` be mandatory for all proposals or only proposals above a risk/complexity threshold?
2. Should follow-up proposal seeds create new Ideas automatically, or remain files until the operator accepts them?
3. Which human role can accept an external blocker boundary: any operator, release owner, or proposal owner?
4. Should provider quota waits be treated as external blockers, or as normal retry scheduling unless they exceed a wall-clock threshold?

