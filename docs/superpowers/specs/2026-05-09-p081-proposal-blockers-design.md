# P081 Proposal Blockers Design

## Summary

This design closes the current `P081` proposal-readiness blockers without expanding into implementation work. The goal is to make Proposal 081 review-ready by turning it from a directionally-correct idea into a concrete, merge-resilient contract for boundary policy ownership, principal-table compatibility, and durable audit behavior.

The design intentionally works with the current repository reality:

- `P081` is blocked in proposal review, not implementation.
- current implemented truth still lives in `docs/reference/*`.
- `P075` is expected to continue moving in `main` while `P081` work is in flight.
- the governed macOS UI must remain a GraphQL read/subscription surface with approval-only mutations.

## Scope

In scope:

- rewrite `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` into a concrete target-state contract;
- resolve Rust review blockers `BLK-RUST-001`, `BLK-RUST-002`, and `BLK-RUST-003`;
- add minimal clarifying edits to adjacent reference docs only where needed to remove false contradictions or stale assumptions;
- make the proposal resilient to a mid-flight merge from `main`, especially after `P075` lands.

Out of scope:

- implementation code changes in Rust, Swift, GraphQL, or MCP;
- changing the current UI action boundary;
- inventing a new command surface for the macOS app;
- broad cleanup of unrelated reference docs;
- reworking `P072`, `P075`, or `P077` beyond the narrow alignment needed for `P081`.

## Product Intent

From a product perspective, `P081` should make the app more predictable:

- the same caller should not be allowed in one surface and denied in another for the same conceptual action;
- the UI should not suggest that an approval action is available when the server will reject it;
- operators should have clearer, more stable denial behavior and diagnostics;
- future UI or automation work should not need to rediscover transport and authorization boundaries from scattered code paths.

## Constraints

## Current-system constraint

The proposal must not rewrite current implemented truth. `docs/reference/*` remains the source of truth for what is already shipped. `P081` must clearly describe a target-state contract and the migration path from current baseline to that target state.

## UI constraint

The governed macOS UI stays approval-only on the write path:

- GraphQL queries and subscriptions are the UI read plane.
- `approveApproval` and `rejectApproval` are the only governed UI mutations.
- MCP remains the operator and automation control plane for all non-approval operational commands.

## Mainline movement constraint

`P081` must survive a mid-flight merge from `main` after `P075` lands. The proposal therefore needs to anchor itself on stable invariants and owned interfaces rather than branch-local wording or implementation details likely to move during durability work.

## Design Decisions

## 1. Proposal 081 becomes a contract-first rewrite

Use Proposal 081 as the primary vehicle for the fix. Do not attempt a surgical patch that only inserts a few blocker responses. The revised proposal must read as one coherent contract rather than a mostly-unchanged document with appended exceptions.

The rewrite should preserve the original product intent, but restructure the proposal around:

- current baseline alignment;
- owned boundary concepts;
- migration and compatibility;
- durable audit semantics;
- review-proof acceptance criteria.

## 2. Separate persisted identity from runtime caller classification

Keep `PrincipalClass` as the canonical persisted identity class. Introduce `CallerClass` as a separate derived classification used for boundary decisions.

Product meaning:

- existing identities do not become ambiguous or retroactively redefined;
- the system gains a clearer way to distinguish the same principal acting through different surfaces;
- future UI and automation behavior can key off a single runtime decision model without redefining storage truth.

Proposal requirements:

- explicitly state that `CallerClass` does not replace `PrincipalClass`;
- define derivation inputs at the contract level: principal identity, transport, surface policy, and request context;
- define how readback compatibility works when both `callerPrincipalClass` and nullable `callerClass` are exposed.

This closes `BLK-RUST-001` and reduces overlap with existing domain/auth types.

## 3. Move the new boundary-aware principal format to schema_version 3

Do not reuse `schema_version: 2` for the new boundary-aware principal format. `schema_version: 2` already has implemented meaning under `P072`.

Required proposal behavior:

- `v1` remains legacy compatibility behavior;
- `v2` remains the current `P072` surface-policy format;
- `v3` is the first format that introduces boundary-aware caller derivation and matrix-linked transport policy;
- malformed or ambiguous `v3` loads fail closed;
- upgrade and defaulting rules from `v2` to `v3` are explicitly documented.

Product meaning:

- older installations and fixtures do not silently change meaning;
- rollout risk is lower because the new rules are opt-in through an explicit schema step;
- support/debug paths can identify which rule-set a principal file is using.

This directly closes `BLK-RUST-002`.

## 4. Give BoundaryPolicy a concrete Rust home and injection path

The proposal must stop talking about `BoundaryPolicy` as a floating concept. It needs an explicit architectural home and dependency shape.

Required target contract:

- `BoundaryPolicy` lives in a shared Rust boundary/auth layer rather than in GraphQL, MCP, or engine-specific code;
- that layer owns fixture loading, validation, evaluator types, and typed decision outputs;
- daemon startup loads one immutable in-memory policy instance;
- the daemon injects that shared instance into GraphQL, MCP, approval actionability consumers, and accepted-command auditing paths.

Product meaning:

- different entrypoints stop making slightly different authorization judgments;
- approval availability can be derived from the same source of truth as mutation denial;
- operators get one stable behavior model instead of transport-specific surprises.

This closes the ownership side of `BLK-RUST-001`.

## 5. Keep audit_log in P081, but make it concrete and phase-aware

Do not remove `audit_log` from Proposal 081. Instead, make it concrete enough that review no longer sees it as hand-wavy or non-implementable.

Required target contract:

- explicit migration boundary for introducing `audit_log`;
- explicit table and field contract at proposal level;
- explicit transaction semantics for when audit writes are required;
- explicit rules for allow-path and deny-path audit behavior;
- explicit fail-closed behavior when audit-required storage is unavailable;
- explicit readback, retention, and integrity expectations.

At the same time, `P081` must not try to re-specify the whole durability framework that `P075` is already shaping. It should define the boundary-specific audit requirements and rely on shared durability invariants where appropriate.

Product meaning:

- sensitive actions and denials become explainable and supportable;
- break-glass and denial cases stop depending on informal logs or best-effort behavior;
- future incidents can be investigated with a clearer source of truth.

This closes `BLK-RUST-003`.

## 6. Add an explicit current-baseline alignment section

Proposal 081 needs a dedicated section that explains how the target contract relates to the current repository truth.

This section should answer, for each disputed area:

- what the current system does now;
- what `P081` adds or tightens;
- what remains compatible;
- which parts are future-state rather than already implemented.

This is the main defense against false contradictions between:

- the proposal;
- current reference docs;
- adjacent proposal slices;
- review assumptions formed from current code.

## 7. Write for post-merge truth, not branch-start truth

Because `P075` may land in `main` during `P081` work, Proposal 081 should be phrased against stable invariants and named interfaces, not branch-local mechanics.

Required authoring rule:

- if shared durability or readback primitives move in `main`, `P081` must adopt the merged naming and ownership model rather than preserve stale wording from the branch start;
- acceptance is against post-merge repository truth;
- the proposal should contain a short note that concurrent durability work may refine shared primitives without changing `P081`'s product or boundary intent.

## Files To Change

## Primary

- `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`

## Minimal adjacent reference edits

- `docs/reference/ui-action-boundary.md`
- `docs/reference/query-projections-and-client-consumption-contract.md`
- `docs/reference/current-system-baseline.md`

Edit these reference docs only when one of these conditions is true:

- the current wording creates a false contradiction with the revised `P081` contract;
- the current wording hides the distinction between implemented baseline and future target state;
- the current wording would become misleading after `P075` lands in `main`.

These reference edits must be clarifying only. They must not claim that `P081` is already implemented.

## Suggested proposal structure

Revise Proposal 081 to include these sections explicitly:

1. Problem and current baseline
2. Boundary concepts and terminology
3. Principal-table compatibility and `schema_version: 3`
4. BoundaryPolicy ownership and injection model
5. Boundary matrix and evaluator contract
6. Audit log durable contract
7. GraphQL, MCP, and approval actionability boundary rules
8. Rollout and compatibility phases
9. Acceptance criteria and proof requirements
10. Current-baseline alignment and coexistence notes

## Acceptance Criteria For This Design Cycle

This design cycle is successful when:

1. Proposal 081 explicitly resolves `BLK-RUST-001`, `BLK-RUST-002`, and `BLK-RUST-003`.
2. The proposal no longer implies that `schema_version: 2` has two incompatible meanings.
3. The proposal clearly distinguishes current implemented truth from future target contract.
4. The proposal preserves the current product boundary where the macOS UI is GraphQL read/subscription plus approval-only mutations.
5. The proposal can absorb a mid-flight merge from `main` after `P075` without requiring a conceptual rewrite.
6. Any reference-doc edits are minimal and only remove ambiguity or contradiction.

## Risks And Mitigations

## Risk: proposal overfits the current branch and goes stale after mainline merge

Mitigation:

- write against invariants, not branch-local mechanics;
- prefer owned interfaces and compatibility rules over low-level implementation narration.

## Risk: audit_log language duplicates or conflicts with P075 durability work

Mitigation:

- scope `P081` audit language to boundary-specific requirements;
- reuse shared durability vocabulary when it already exists;
- avoid creating parallel names for the same durability primitive.

## Risk: reference docs accidentally imply implementation is already complete

Mitigation:

- keep reference edits narrowly clarifying;
- preserve the distinction between current baseline and proposal target state.

## Review Checklist

- Does the proposal name one clear architectural home for `BoundaryPolicy`?
- Does it make `schema_version: 3` the only new boundary-aware principal format?
- Does it keep `PrincipalClass` and `CallerClass` distinct?
- Is the `audit_log` contract concrete enough to be implemented without unstated storage decisions?
- Does the proposal preserve the UI approval-only write boundary?
- Could the document still read as coherent after `P075` lands in `main`?

## Recommended Next Step

After this spec is approved, the next step is to produce a writing-plans implementation plan for the actual document edits, review loop, and post-edit validation sequence.
