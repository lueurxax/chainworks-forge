# Proposal 062: Implementation Approval Rejection Loopback

| Field | Value |
|---|---|
| Date | 2026-04-20 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md#implementation-self-assessment-and-handoff), [full-mvp-delivery.md](../reference/full-mvp-delivery.md), [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md) |
| Scope | Make rejected implementation approval a normal full-MVP workflow loopback into proposal refinement/review, with durable operator feedback and proof that implementation does not start from a rejected approval. |
| Goal | Let operators reject stale or unsuitable implementation approval requests and return the proposal to the Review <-> Refine loop without manual database repair, recovery-only tools, or accidental implementation start. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-062|p062`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

During dogfooding on 2026-04-20, P031 reached `state_6_implementation_approval` after review. The operator then materially changed the proposal scope: the macOS UI must be GraphQL-only for reads, must not use MCP from UI, and must not add write paths except approval-related controls under a separately approved transport.

The correct product action was to reject the stale implementation approval and send the proposal back through refinement and review. The current full-MVP workflow could not do that directly:

- `state_6_implementation_approval` only transitions on `approval.granted == true`;
- `approval.rejected == true` is not a supported workflow condition;
- rejecting the approval blocks the stage/run;
- returning to `state_5_proposal_refined` required manual DB repair followed by `stages.retry`.

The implemented full-MVP and artifact-contract references now record the implementation self-assessment and handoff semantics. P062 exists as a dedicated follow-up for any remaining approval-rejection loopback hardening so that behavior is not lost in future workflow changes.

---

## 2. Problem Statement

### 2.1 Rejected implementation approval is treated as a blocked terminal path

Rejecting implementation approval means "this reviewed proposal should not be implemented yet." It does not mean the run is unrecoverable.

The current engine resolves the approval as rejected and blocks the stage. Operators then need manual repair to move the run pointer back to `state_5_proposal_refined`.

### 2.2 Workflow condition language is asymmetric

The condition evaluator supports `approval.granted == true` but not `approval.rejected == true`. That makes rejection impossible to model as a normal workflow transition.

### 2.3 Rejection feedback is not routed into the next refinement pass

The approval rejection comment should become durable refinement input. Without explicit routing, the next proposal writer pass may not see why approval was rejected.

---

## 3. Scope

P062 includes:

- `approval.rejected == true` workflow condition support.
- A full-MVP transition from `state_6_implementation_approval` to `state_5_proposal_refined`.
- Durable propagation of the rejected approval comment into the next refinement attempt context.
- Guardrails preventing `state_7_implementation_started` and implementation worktree provisioning after rejected implementation approval.
- Projection/readback evidence that the run is looping by operator rejection rather than failing due to provider/runtime error.
- Focused tests and a `proposal-062|p062` validation gate.

P062 does not include:

- A new UI write path for approval decisions.
- GraphQL mutations for approval decisions.
- UI use of MCP tools. MCP remains for agents, CLI/operator diagnostics, automation, and debug/control outside the macOS UI.
- General approval re-arm tooling; `approvals.rearm` remains P045 scope.
- Release approval override semantics; release gates remain P059 scope.
- Reworking the stable implementation self-assessment and handoff contract.

---

## 4. Proposed Behavior

### 4.1 Workflow transition

Update `examples/workflows/full-mvp-live.yaml` and `examples/workflows/workflow.yaml`:

```yaml
state_6_implementation_approval:
  transitions:
    - to: state_7_implementation_started
      when: approval.granted == true
    - to: state_5_proposal_refined
      when: approval.rejected == true
```

The rejected path is not a recovery command. It is the normal workflow interpretation of an operator saying that implementation should not begin from the current proposal/review state.

### 4.2 Condition evaluator

The engine transition evaluator must support:

- `approval.granted == true`
- `approval.rejected == true`

The evaluator must scope the condition to the current run and relevant approval stage. A rejected approval for an older stage must not satisfy a later stage's transition unless the workflow explicitly targets that stage's active approval.

### 4.3 Rejection context

When `state_6_implementation_approval` is rejected:

- the approval row remains durable with `decision = rejected`, `comment`, `requested_at`, `decided_at`, and `stage_id`;
- command journal records the rejection;
- the next `state_5_proposal_refined` execution receives a structured rejection context containing the rejected stage id, approval id, comment, and prior proposal/review artifact references;
- `proposal_writer` is instructed to treat that rejection as operator feedback and produce a new proposal revision;
- the subsequent `state_4_proposal_reviewed` pass runs normally and creates fresh review artifacts;
- a later `state_6_implementation_approval` request is fresh and distinct from the rejected one.

### 4.4 Implementation-start guard

After rejection:

- `state_7_implementation_started` must not run;
- no implementation worktree should be provisioned by that rejected approval;
- no `code_writer` work item should be created from that rejected approval;
- run status should be `running` or queued/backpressured for `state_5_proposal_refined`, not permanently `blocked`, unless proposal loop budget is exhausted.

---

## 5. Implementation Inventory

- `examples/workflows/full-mvp-live.yaml`
- `examples/workflows/workflow.yaml`
- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/workflow/tests/integration.rs`
- `control-plane/crates/engine/src/orchestrator.rs`
- `control-plane/crates/engine/src/command_handler.rs`
- `control-plane/crates/engine/tests/integration.rs`
- `control-plane/crates/db/src/repos/approvals.rs`
- `control-plane/crates/db/src/repos/run_state_projections.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`
- `docs/reference/full-mvp-delivery.md`
- `docs/reference/proposal-loop-feedback-fidelity-and-rereview.md`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

---

## 6. Tests and Proof Gate

Add a canonical gate alias:

- `proposal-062`
- `p062`

Required proof:

- Condition evaluator test proves `approval.rejected == true` matches rejected approvals and does not match granted/requested approvals.
- Workflow compiler test proves full-MVP workflow accepts both granted and rejected transitions from `state_6_implementation_approval`.
- Engine integration test proves rejecting `state_6_implementation_approval` transitions the run to `state_5_proposal_refined`.
- Engine integration test proves the rejection comment is available to the next proposal refinement execution.
- Regression test proves rejected implementation approval does not create `state_7_implementation_started`, implementation worktree, or `code_writer` work items.
- Fresh-approval test proves the next successful review creates a new implementation approval request distinct from the rejected approval.
- Projection/API tests prove MCP and GraphQL readback expose rejection-loopback state without requiring clients to infer it from raw DB rows.
- Gate registry tests prove `proposal-062|p062` is discoverable.

---

## 7. Rollout

1. Add evaluator support for `approval.rejected == true` with focused unit tests.
2. Add workflow transitions in both full-MVP workflow files.
3. Add rejection context plumbing into refinement inputs.
4. Add implementation-start guard tests.
5. Add projection/readback fields or status reason if current projections cannot explain rejection loopback.
6. Register `proposal-062|p062` gate.

---

## 8. Acceptance Criteria

- Operators can reject implementation approval and the run returns to proposal refinement/review without manual DB edits.
- Rejection comment is durable and visible to the next proposal writer pass.
- Rejected implementation approval cannot start implementation or provision an implementation worktree.
- The next implementation approval request is fresh and distinct.
- MCP/GraphQL readback can explain that the run is looping due to operator rejection.
- `./scripts/test-gate.sh proposal-062` passes.
