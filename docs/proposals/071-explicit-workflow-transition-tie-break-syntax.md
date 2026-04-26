# Proposal 071: Explicit Workflow Transition Tie-Break Syntax

| Field | Value |
|---|---|
| Date | 2026-04-24 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md](017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md), [workflow-execution-engine.md](../reference/workflow-execution-engine.md), [yaml-dsl-parser.md](../reference/yaml-dsl-parser.md) |
| Scope | Add an explicit, compiled, auditable transition tie-break mechanism for workflow states whose declarative transition conditions can intentionally overlap. |
| Goal | Let workflow authors resolve intentional multi-match transitions without returning to implicit first-match behavior, agent-authored routing hints, or hidden YAML ordering semantics. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-071|p071`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context

P017 makes the compiled workflow graph authoritative and fail-closed. Under P017, when multiple declarative transitions from the same state match at runtime, the engine persists `multiple_declarative_transitions_matched_without_tie_break` and blocks the run. That is the correct default because implicit first-match behavior can hide workflow bugs.

Some workflows still need intentional overlap. Common examples:

- a high-specificity failure transition and a general fallback transition;
- a manual-approval rejection path and a broader "not approved" path;
- a terminal safety transition that should win over a lower-priority continuation path;
- a degraded-output path that should win only when a normal success path is absent.

P071 adds a deliberate syntax for those cases. The syntax must make the author decision visible in YAML, compiled plan output, candidate transition diagnostics, workflow conflict records, and tests.

---

## 2. Non-Negotiable Rules

- P017 remains the default: multi-match without an explicit tie-break blocks with a typed workflow conflict.
- YAML list order is never a tie-break.
- Agent-authored `next_stage`, `next_action`, run_state artifacts, and narrative hints are never tie-break inputs.
- A tie-break may select only among already-matched compiled transitions.
- A tie-break cannot create a state, bypass an approval gate, or override missing/invalid transition inputs.
- Tie-break decisions must be visible in `CandidateTransitionEvaluation`, transition cursor truth, workflow conflict/advisory history, and GraphQL/MCP readback.

---

## 3. Proposed YAML Syntax

Each transition may declare a stable `id` and an optional `selection_priority`:

```yaml
states:
  review:
    transitions:
      - id: review_failed_to_refinement
        to: refine
        when: "proposal_review_summary.pass == false"
        selection_priority: 200

      - id: review_passed_to_approval
        to: approval
        when: "proposal_review_summary.pass == true"
        selection_priority: 100

      - id: review_unclassified_to_lead
        to: lead_conflict_mediation
        when: "exists('proposal_review_summary')"
        selection_priority: 10
```

Selection rule:

- Evaluate all transitions exactly as P017 defines.
- If zero transitions match, keep existing no-match behavior.
- If one transition matches, select it.
- If multiple transitions match:
  - every matched transition must have a non-null `selection_priority`;
  - matched priorities must be unique;
  - the transition with the highest priority wins;
  - the transition cursor records `selection_reason=explicit_priority_tie_break`.
- If any matched transition lacks priority, or two matched transitions share the same priority, block with `multiple_declarative_transitions_matched_without_tie_break`.

The proposal intentionally chooses `selection_priority` rather than `order` or `rank` because it is a declarative selection property, not a promise about YAML sequence order.

---

## 4. Data Model

### 4.1 Workflow definition

Extend `workflow::definition::Transition`:

```rust
pub struct Transition {
    pub id: Option<String>,
    pub to: String,
    pub when: String,
    pub selection_priority: Option<i64>,
}
```

Validation rules:

- `id` is optional for legacy transitions but required when `selection_priority` is present.
- `id` must be unique within a source state.
- `selection_priority` must be an integer in the supported range chosen by implementation.
- A state may mix prioritized and unprioritized transitions, but any runtime multi-match involving an unprioritized transition remains a conflict.

### 4.2 Compiled plan

Extend `workflow::plan::CompiledTransition`:

```rust
pub struct CompiledTransition {
    pub id: String,
    pub to: String,
    pub condition: String,
    pub selection_priority: Option<i64>,
}
```

For legacy transitions without an explicit `id`, the compiler may preserve the current generated id shape as compatibility metadata. New authoring guidance should require explicit IDs for any state that needs tie-break behavior.

### 4.3 Candidate transition diagnostics

Extend `CandidateTransitionEvaluation` with:

- `transition_id`;
- `selection_priority`;
- `selection_group` reserved for future use, always null in P071;
- `selected_by_tie_break`;
- `tie_break_diagnostic`.

The selected transition cursor must record:

- `selected_transition_id`;
- `selected_next_state_id`;
- `selection_reason=explicit_priority_tie_break` when priority resolved a multi-match;
- `candidate_transition_hash`;
- all matched candidates and their priorities.

---

## 5. Runtime Behavior

P071 changes only the branch where more than one transition matched.

Current P017 behavior:

```text
matched_count > 1 -> workflow_conflict
```

P071 behavior:

```text
matched_count > 1
  if all matched transitions have distinct selection_priority
    choose max(selection_priority)
    record explicit tie-break cursor/readback
  else
    workflow_conflict
```

The engine must not partially sort by priority and then silently fall back to YAML order. Missing or duplicate priority is an error condition, not a warning.

---

## 6. Readback

GraphQL and MCP readback must expose tie-break decisions in operator-safe terms:

- selected transition id;
- selected next state id;
- selected priority;
- matched candidate count;
- non-selected matched transition ids and priorities;
- `selection_reason=explicit_priority_tie_break`.

No hidden reasoning, prompts, or agent rationale is involved. This is workflow graph metadata only.

---

## 7. Migration

P071 does not require editing every workflow immediately.

- Existing workflows without multi-match continue to behave as before.
- Existing workflows with accidental multi-match continue to block under P017 until explicitly re-authored.
- Bundled workflows known to rely on implicit first-match behavior must either:
  - be re-authored with mutually-exclusive conditions;
  - add explicit `id` and `selection_priority`; or
  - remain covered by a known-issues migration record until re-authored.
- External legacy catalogs follow the P017 Phase C warning-window policy before fail-closed enforcement.

---

## 8. Acceptance Criteria

- **AC-1.** Workflow YAML accepts `id` and `selection_priority` on transitions.
- **AC-2.** Compiler output includes stable transition ids and optional priorities.
- **AC-3.** Multi-match with all distinct priorities selects the highest priority transition and records `explicit_priority_tie_break`.
- **AC-4.** Multi-match with a missing priority still blocks with `multiple_declarative_transitions_matched_without_tie_break`.
- **AC-5.** Multi-match with duplicate highest priorities blocks with `multiple_declarative_transitions_matched_without_tie_break`.
- **AC-6.** YAML order changes do not change selected transition when priorities are unchanged.
- **AC-7.** Candidate transition diagnostics, transition cursor, MCP readback, and GraphQL readback expose the selected transition and non-selected matched candidates.
- **AC-8.** Agent-authored advisory next_stage/next_action cannot influence tie-break selection.
- **AC-9.** `./scripts/test-gate.sh proposal-071` passes and is documented in `docs/reference/test-gates.md`.

---

## 9. Non-Goals

- No probabilistic, score-based, or lead-mediated transition selection.
- No automatic inference of priority from YAML order.
- No new workflow states.
- No runtime mutation of workflow transition definitions.
- No UI work beyond GraphQL/MCP readback needed by a future thin-client surface.

---

## 10. Open Questions

1. Should P071 reserve `selection_group` now for future grouped tie-breaks, or keep the syntax to `selection_priority` only until a real grouped use case exists?
2. Should priority use highest-wins as proposed, or lowest-wins to match some scheduler conventions?
3. Should explicit `id` become required for all transitions after a future migration window, even when no priority is present?
