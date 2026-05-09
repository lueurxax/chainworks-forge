# Proposal 085: Thin-Client Read-Model Parity and Affordance Contract

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Implemented — see [reference/thin-client-read-model-affordance-contract.md](../reference/thin-client-read-model-affordance-contract.md) |
| Author | Codex |
| Depends on | [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md), [ui-action-boundary.md](../reference/ui-action-boundary.md) |
| Related | P036 UI restoration, P068/P081 boundary matrix, Swift `P031ThinGraphQLReadBoundary.swift`, Swift `RunsHomeView.swift` |
| Scope | Require every GraphQL field that drives a Swift affordance to define actionable state, disabled reason, fallback text, mutation availability, and preview/readback semantics together. |
| Goal | Prevent thin-client UI drift where backend read models, actionability, fallback copy, and mutation policy are reviewed in separate passes. |

---

## 1. Problem

The thin UI now correctly avoids local workflow truth, but recent UI/debug work exposed a new class of parity problems:

- a Swift row can show a stale list-level render state while a detail preview has already loaded a richer payload;
- `Live` freshness can be visually confused with payload/action availability;
- approval actionability fields can contradict backend mutation authorization;
- GraphQL schema/read-model changes and Swift affordance text often move together but are not reviewed as one contract.

Thin-client safety is not enough. The UI also needs read-model parity: every affordance must be driven by a server-owned field or an explicit local presentation state.

## 2. Decision

Whenever a GraphQL field drives a Swift affordance, the proposal must define the full affordance contract in one place.

This applies to:

- approval buttons;
- retry/cancel/start visibility;
- artifact preview state;
- report payload state;
- freshness badges;
- disabled/fallback copy;
- diagnostic copy actions;
- any future approval-only mutations.

## 3. Affordance Contract

Add canonical artifact:

```text
docs/reference/thin-client-read-model-affordance-contract.md
```

Required per affordance:

| Field | Required definition |
|---|---|
| source GraphQL field(s) | exact query/subscription field and nullability |
| local presentation state | any Swift-only state such as selected row or preview cache |
| actionable state | server-owned or explicit local-only state |
| disabled reason | enum/code and fallback text |
| mutation availability | allowed mutation, denied mutation, or no mutation |
| stale/list/detail behavior | what happens when list data is less complete than detail data |
| unauthorized behavior | redaction/denial/fallback |
| tests | backend projection + Swift presenter/render expectation |

## 4. Required Behavior

### 4.1 List/detail state merging

If list rows intentionally omit payload or detail fields, the UI must not label the row as permanently unavailable.

Allowed labels:

- `Open to preview` for payloads that are available but not loaded in list;
- concrete type labels such as `JSON`, `Markdown`, `Diff`, `Text` after preview load;
- `No preview` only when the server says payload/rendering is unavailable;
- `Metadata` or `Metadata only` when only metadata is intentionally exposed.

### 4.2 Freshness is not availability

Freshness badges must communicate projection recency only. They must not be used as payload availability, actionability, or permission signals.

### 4.3 Approval actionability

Approval rows must present action buttons only when:

- durable approval state is actionable;
- caller policy allows the approval mutation;
- mutation availability is `approveApproval`/`rejectApproval` only;
- disabled reason and fallback text match backend authorization.

### 4.4 Mutation availability reviewed with reads

No Swift affordance may infer mutation availability from display text, status label, freshness state, or local selection alone.

## 5. Tests

Add proof gate:

```text
proposal-085|p085
```

Required tests:

- GraphQL projection fixture for each affordance state;
- Swift presenter test for label/fallback mapping;
- UI render snapshot or view-state test for list/detail payload merge;
- approval actionability test tied to P081 boundary matrix;
- unauthorized/redacted readback test where applicable.

## 6. Non-Goals

- Do not add broad UI writes.
- Do not make Swift local state authoritative for workflow truth.
- Do not force payloads into bulk list queries.
- Do not replace P036 visual/navigation restoration; this proposal defines the contract P036 must consume.

## 7. Acceptance Criteria

P085 is complete when:

1. the affordance contract exists under `docs/reference/`;
2. artifact preview, approval actionability, freshness, and report payload states have explicit contract rows;
3. Swift rows merge list/detail preview state without misleading `Unavailable` labels;
4. future UI affordance changes cite the contract and include backend + Swift presenter tests.
