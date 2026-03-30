# Proposal 017: Lead-Mediated Workflow Conflict Resolution and Mandatory Lead Validation

| Field | Value |
|---|---|
| Date | 2026-03-30 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/workflow-execution-engine.md](../reference/workflow-execution-engine.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/yaml-dsl-parser.md](../reference/yaml-dsl-parser.md), [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md), [reference/current-system-baseline.md](../reference/current-system-baseline.md) |
| Scope | Declarative workflow-authority enforcement, lead-mediated conflict resolution for invalid or unresolved workflow progression, mandatory lead presence in the agent catalog, and fail-closed validation for workflows that cannot escalate conflicts to a lead |
| Goal | Ensure the runtime never silently guesses or deadlocks when workflow truth becomes ambiguous: it must either follow a valid declarative transition or escalate the conflict to a system lead with a durable operator-visible resolution path. |

---

## 1. Context

Run `D4F404B7-8D3D-483A-956E-5C95F201FD63` exposed a workflow conflict that should not have been left to raw agent output or silent blocking.

The proposal-review aggregate output clearly showed:

- `average_score = 9.0`
- `aggregate_score = 36`
- `blocker_count = 1`
- `pass = false`

That means the live workflow should have looped back into proposal refinement.

Instead, the persisted `run_state` wrote:

- `next_action = "revise_proposal"`
- `next_stage = "state_3_proposal_drafted"`

That stage does not exist in the current declarative workflow.

This incident proves two product problems:

1. the runtime still allows agent-authored workflow progression hints to drift away from declarative workflow truth;
2. when workflow truth becomes ambiguous, the engine blocks or degrades instead of escalating the conflict through an explicit lead-owned decision path.

Proposal 017 addresses those two problems directly.

### 1.1 Why this needs a separate proposal

The implemented [execution-truth and recovery baseline](../reference/execution-truth-and-recovery.md) repairs execution truth, settlement truth, and resume idempotency.
Proposal 013 repairs output-contract truth, failure evidence, and narrow recovery.

Neither proposal makes one architectural promise that the host system still needs:

- when workflow progression is invalid, conflicting, or unresolved, a lead must own the resolution path.

That is a workflow-authority and governance slice, not a transport or output-contract slice.

### 1.2 What this proposal is not

Proposal 017 is **not**:

- a general multi-lead delegation redesign,
- a new scoring proposal,
- a replacement for declarative transition evaluation,
- a runtime audit proposal,
- or a UI-polish proposal.

It is specifically about:

- preserving declarative workflow authority,
- routing workflow conflicts to a lead,
- and making the existence of that lead a compile-time/runtime requirement rather than an incidental convention.

---

## 2. Product questions this proposal must answer

After Proposal 017, the engineer must be able to answer all of these with persisted evidence rather than inference:

1. If an agent writes an invalid `next_stage` or otherwise proposes a transition that is not legal in the workflow graph, does the engine reject it and keep declarative workflow truth authoritative?
2. If no declarative transition matches the current state because runtime data is contradictory or insufficient, does the engine escalate to a lead-owned resolution path instead of silently blocking?
3. Does every executable workflow have a guaranteed lead escalation path, or does validation fail before runtime?
4. Can the operator see that a run is blocked because of a workflow conflict, which lead owns the conflict, and what the valid next actions are?
5. Can workflows without a lead be rejected deterministically at validation time rather than failing later at runtime?

Proposal 017 is done only when all five answers are explicit in the persisted model, operator surfaces, validation, and test evidence.

---

## 3. What we build

Proposal 017 delivers four tightly coupled layers.

### Layer W: Declarative Workflow Authority

| Component | Responsibility |
|---|---|
| **TransitionAuthorityResolver** | Treats the compiled workflow graph as the only authority for stage progression; agent-authored `next_stage` becomes advisory evidence only |
| **WorkflowConflictClassifier** | Classifies invalid-next-stage, no-transition-match, conflicting-transition, and unresolved-runtime-input cases into explicit workflow-conflict reasons |
| **WorkflowConflictRecord** | Persists the conflict reason, current state, candidate transitions, advisory agent hints, and the lead owner responsible for resolving it |

### Layer X: Lead-Mediated Conflict Resolution

| Component | Responsibility |
|---|---|
| **SystemLeadResolver** | Resolves the canonical lead agent for the workflow/run from catalog truth rather than ad hoc stage ownership |
| **LeadConflictMediationStep** | Creates a first-class lead-owned mediation step when workflow truth becomes ambiguous or invalid |
| **LeadResolutionContract** | Defines the machine-readable output a lead must produce to resolve a workflow conflict: chosen action, chosen state, rationale, and whether operator approval is still required |

### Layer Y: Mandatory Lead Validation

| Component | Responsibility |
|---|---|
| **LeadPresenceValidator** | Fails catalog/workflow validation when no system lead is declared and the workflow cannot escalate conflicts safely |
| **LeadRoleSchema** | Adds explicit catalog truth for the system lead role rather than relying only on the string id `lead_orchestrator` |
| **WorkflowLeadCoverageGate** | Verifies that each workflow using the catalog either inherits the system lead or explicitly declares an equivalent lead escalation path |

### Layer Z: Operator Surfaces

| Component | Responsibility |
|---|---|
| **WorkflowConflictPanel** | Explains the workflow conflict, the declarative state involved, the advisory agent hint that was rejected, and the lead-owned next action |
| **LeadEscalationRecoveryAction** | Lets the operator trigger or inspect lead mediation without cloning the run unnecessarily |
| **WorkflowConflictReportBridge** | Surfaces workflow-conflict classification and lead resolution in run reports instead of silent `blockedReason = null` states |

---

## 4. Declarative workflow remains authoritative

### 4.1 Current defect

The runtime currently tolerates a dangerous ambiguity:

- agent or aggregate artifacts can write a `next_stage`,
- the workflow graph separately defines valid transitions,
- and when these disagree, the engine can end up blocked without an explicit conflict owner.

That makes the workflow graph non-authoritative in practice.

### 4.2 Authority rule

Proposal 017 makes this explicit:

1. The compiled workflow graph is the only authority for legal progression.
2. Agent-authored fields like `next_stage`, `next_action`, or narrative transition hints are advisory evidence only.
3. Advisory transition hints may:
   - help the lead explain the conflict,
   - be shown in reports/recovery surfaces,
   - or be used for diagnostics.
4. Advisory transition hints may **not**:
   - advance the run on their own,
   - override a declarative transition result,
   - or create synthetic stages not present in the workflow graph.

### 4.3 Conflict classes

The runtime must classify at least these workflow conflicts:

- `invalid_next_stage_hint`
- `no_declarative_transition_matched`
- `multiple_declarative_transitions_matched_without_tie_break`
- `required_artifact_or_field_missing_for_transition`
- `aggregate_transition_truth_conflicted`
- `workflow_conflict_unverifiable`

Each conflict must be persisted as a `WorkflowConflictRecord`.

---

## 5. Lead-mediated workflow conflict resolution

### 5.1 Core rule

When workflow truth is ambiguous, invalid, or unresolved:

- the engine must not guess,
- the engine must not silently block without an owner,
- and the engine must not immediately force a clone path if a same-run lead decision remains valid.

Instead, it must escalate to the lead.

### 5.2 Lead mediation contract

The lead must resolve workflow conflicts through an explicit machine-readable contract.

Minimum required fields:

- `conflict_id`
- `current_state_id`
- `conflict_reason`
- `resolution_mode`
- `chosen_action`
- `chosen_next_state_id`
- `requires_operator_confirmation`
- `rationale`

Rules:

1. `chosen_next_state_id` must be one of:
   - a legal next state from the declarative graph,
   - the current state for same-state retry/re-entry,
   - or `null` when the correct action is approval, operator decision, or clone.
2. The lead may not invent a state that is absent from the compiled workflow graph.
3. If no legal same-run action exists, the lead must say so explicitly and point the operator to clone or manual resolution.

### 5.3 When lead mediation is automatic vs operator-triggered

Default behavior:

- the engine creates the conflict record automatically,
- the lead mediation step is created automatically if a lead exists and the run is otherwise resumable,
- the operator remains the final owner only when:
  - manual approval is still required,
  - the conflict is marked `workflow_conflict_unverifiable`,
  - or the lead concludes no safe same-run path exists.

---

## 6. Mandatory lead validation

### 6.1 Why the lead must be mandatory

Today, `lead_orchestrator` exists by convention in the example catalog and workflows.
Proposal 017 turns that convention into validation truth.

Without a lead, the system has no bounded owner for:

- workflow conflicts,
- aggregate-governance conflicts,
- unresolved transition intent,
- or safe escalation short of human cloning/manual intervention.

### 6.2 Required schema rule

The catalog must explicitly declare one system lead.

This proposal does not require the id to remain literally `lead_orchestrator`, but it does require an explicit role marker, for example:

- `system_role: lead`

Validation rules:

1. exactly one system lead must exist in the catalog;
2. workflows using that catalog must either:
   - inherit that lead automatically,
   - or explicitly map to an equivalent lead agent if the design later allows it;
3. a catalog with zero leads fails validation;
4. a catalog with multiple system leads fails validation unless a later proposal introduces a bounded multi-lead model;
5. `agents.yaml` without a lead must not pass validation.

### 6.3 Runtime fallback rule

If a legacy catalog somehow reaches runtime without a validated lead:

- the run must fail closed into `needsDecision`,
- the conflict must be persisted as `workflow_conflict_unverifiable`,
- and operator surfaces must explain that no valid lead escalation path exists.

The runtime must not silently continue.

---

## 7. Persistence and operator surfaces

### 7.1 WorkflowConflictRecord

At minimum, the persisted conflict record must capture:

- `conflictID`
- `runID`
- `stageExecutionID`
- `lineageID`
- `currentStateID`
- `reason`
- `candidateTransitions`
- `advisoryNextStageHint`
- `advisoryNextAction`
- `leadAgentID`
- `createdAt`
- `resolvedAt`
- `resolutionRecordJSON`

### 7.2 Operator surfaces

Blocked-run and report surfaces must show:

- that the block is a workflow conflict rather than generic failure,
- which declarative state is affected,
- which advisory hint was rejected,
- which lead owns the mediation,
- and whether the valid next action is:
  - same-run continue,
  - same-state retry,
  - approval,
  - or clone/manual intervention.

`blockedReason = null` is explicitly not acceptable for this class.

---

## 8. Implementation plan

### 8.1 Phase A: authority and conflict truth

1. Make `TransitionAuthorityResolver` ignore agent-authored `next_stage` as an authority channel.
2. Add `WorkflowConflictClassifier` and `WorkflowConflictRecord`.
3. Persist conflict truth whenever no legal declarative transition can be chosen.
4. Surface workflow-conflict truth in reports and recovery surfaces.

### 8.2 Phase B: lead mediation

1. Add `LeadRoleSchema` and `SystemLeadResolver`.
2. Add `LeadConflictMediationStep` and `LeadResolutionContract`.
3. Route workflow-conflict runs through lead mediation before broad operator clone fallback where safe.

### 8.3 Phase C: fail-closed validation

1. Add `LeadPresenceValidator`.
2. Make `agents.yaml` without a lead fail validation.
3. Add `WorkflowLeadCoverageGate` for workflow/catalog combinations.

Phase A closes the current bug class.
Phases B and C turn it into a stable system rule.

---

## 9. Acceptance criteria

Proposal 017 is complete only when all of the following are true:

1. An invalid agent-authored `next_stage` can no longer advance a run or silently deadlock it.
2. If no declarative transition matches, the runtime persists a `WorkflowConflictRecord` instead of generic blocking noise.
3. Reports and recovery surfaces show the workflow conflict and the lead owner explicitly.
4. A workflow conflict with a valid same-run resolution path escalates to the lead before broad clone fallback.
5. `agents.yaml` without an explicit system lead fails validation.
6. A workflow/catalog pair that cannot escalate workflow conflicts safely fails validation.
7. The motivating replay class for `D4F404B7-8D3D-483A-956E-5C95F201FD63` no longer blocks with a null or misleading reason.

---

## 10. Verification

Minimum proof required:

1. unit tests showing declarative transitions outrank advisory `next_stage`;
2. regression test for the motivating class:
   - aggregate review summary says refine loop,
   - advisory `next_stage` points to a non-existent state,
   - runtime still chooses declarative refine loop or persists a workflow conflict instead of silently blocking;
3. validation tests proving catalogs without a lead fail;
4. report/recovery tests proving workflow conflicts surface a lead-owned action instead of `blockedReason = null`.

---

## 11. Risks and tradeoffs

- Making the lead mandatory increases catalog strictness, but the alternative is leaving workflow-conflict ownership implicit.
- Lead mediation adds one more explicit runtime step, but that is preferable to silent deadlocks or invented next stages.
- Some current or future lightweight workflows may not feel like they need a lead; this proposal deliberately rejects that convenience in favor of fail-closed governance.

---

## 12. Recommendation

Adopt Proposal 017 after the current transition fix lands.

The immediate workflow bug should still be fixed at the declarative source when possible, as in the motivating run.
But the system also needs a general rule:

- declarative workflow truth is authoritative,
- workflow conflicts are first-class runtime objects,
- and the lead is the bounded owner for resolving them.
