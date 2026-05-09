# Thin-Client Read-Model Affordance Contract

**Contract schema:** `thin_client_affordance_contract_v1`  
**Status:** Implemented  
**Proposal:** P085 — Thin-Client Read-Model Parity and Affordance Contract  
**Revision:** p085-r2-2026-05-08

---

## Overview

The governed macOS app is a thin GraphQL read-side client plus the two approval mutations
(`approveApproval`, `rejectApproval`) allowed by `docs/reference/ui-action-boundary.md`.
This document is the canonical, implemented-system contract for every SwiftUI affordance
driven by GraphQL read models. Each row names source fields, local presentation state,
actionability, disabled and fallback copy, mutation availability, mutation conflict behavior,
stale list/detail behavior, unauthorized behavior, staleness deadlines, cancellation policy,
supported interactions, and proof tests.

Swift affordance mapping is centralized in `P085AffordancePresenter` as immutable, Equatable,
Sendable DTOs. SwiftUI views consume tested affordance states rather than raw GraphQL strings.

---

## Compatibility Policy

| Change type | Policy |
|---|---|
| Additive GraphQL fields | Allowed when optional or nullable for old clients and covered by SDL/introspection proof and Swift decoding tests |
| Enum value additions | Allowed when Swift has unknown-case handling that fails closed to diagnostic/unknown and never maps to optimistic actionability |
| Fallback copy changes | Allowed when disabled reason codes remain stable; copy-only changes must update presenter snapshot/render tests and accessibility help text tests |
| Persisted projection state | If implementation needs stored fields for payload deadlines, stall reasons, or typed action availability, the rollout contract migrations section must be revised before implementation freeze |
| Removed or retyped fields | Breaking change — requires explicit proposal revision, gate update, and migration-contract update if persisted projection shape changes |
| Renamed affordance IDs | Breaking change — must retain old id as deprecated alias for one contract version or update all tests, accessibility identifiers, P036 citations, and negative fixtures before freeze |

---

## Row Schema Columns

Each required row includes:

- `affordance_id` — stable identifier for the affordance (breaking change to rename)
- `source_graphql_fields` — GraphQL fields that drive this affordance
- `local_presentation_state` — Swift-local DTO enum/struct consumed by SwiftUI
- `actionable_state` — when the affordance is enabled
- `disabled_reason_code` — `P031DisabledReasonCode` value when disabled
- `fallback_text` — stable copy for the terminal disabled state
- `mutation_availability` — allowed mutations or `none` / `external_transport_only`
- `mutation_idempotency` — conflict and duplicate-submit behavior for mutations
- `staleness_deadline` — server-owned deadline policy for generating/deferred states
- `cancellation_policy` — when in-flight async requests are cancelled
- `stale_list_detail_behavior` — how stale detail state relates to list state
- `unauthorized_behavior` — what happens on unauthorized reads
- `supported_interactions` — surfaces where this affordance appears
- `proof_tests` — Swift test identifiers and fixture files

---

## Required Rows

### `artifact.preview.listLabel`

| Field | Value |
|---|---|
| **affordance_id** | `artifact.preview.listLabel` |
| **source_graphql_fields** | `artifacts(runId:)`, `payloadAvailabilityState`, `payloadUnavailableReasonCode`, `freshnessState`, `diagnosticId`, `serverDebugDetail` |
| **local_presentation_state** | `P085ArtifactAffordanceState.PayloadPresentation` — `.available`, `.metadataOnly`, `.deferred`, `.generating`, `.unavailable(reasonCode:)`, `.unknown(rawState:)` |
| **actionable_state** | List row always visible; keyboard default open and Quick Look enabled only for `.available`; keyboard open enabled for `.deferred` |
| **disabled_reason_code** | n/a (list label is read-only) |
| **fallback_text** | `.deferred` → "Open to preview"; `.metadataOnly` → "metadata only"; `.unavailable` → "Unavailable"; `.unknown` → "Status Unknown" |
| **mutation_availability** | none |
| **staleness_deadline** | `generating` and `payload_deferred` require server-owned deadline or explicit no-deadline justification; stalled transition yields typed stalled/timed-out diagnostic |
| **cancellation_policy** | n/a (read-only list label) |
| **stale_list_detail_behavior** | List label comes from list query; detail may merge richer state over list for selected artifact only; stale async detail responses do not overwrite a newer selection |
| **unauthorized_behavior** | `payloadUnavailableReasonCode: NOT_AUTHORIZED` renders `.unavailable(reasonCode: .notAuthorized)`; Swift does not fall back to local storage or filesystem |
| **supported_interactions** | `list_row`, `context_menu`, `keyboard_default_open` (available/deferred only), `quick_look_when_detail_authorized` (available only) |
| **proof_tests** | `Proposal085Tests/payloadDeferredMapsToDeferred`, `payloadDeferredLabelIsNotUnavailable`, `metadataOnlyMapsToMetadataOnly`, `unavailablePayloadPreservesReasonCode`, `unknownPayloadStateFailsClosed`, `availablePayloadIncludesQuickLook`, `deferredPayloadIncludesKeyboardButNotQuickLook`, `unavailablePayloadExcludesInteractiveOptions` |

---

### `artifact.preview.detail`

| Field | Value |
|---|---|
| **affordance_id** | `artifact.preview.detail` |
| **source_graphql_fields** | `artifact(id:)` or current artifact detail entrypoint, `payloadText`, `payloadAvailabilityState`, `payloadUnavailableReasonCode`, `disabledReasonCode`, `diagnosticId`, `serverDebugDetail` |
| **local_presentation_state** | `P085ArtifactAffordanceState.PayloadPresentation` — richer than list; detail may upgrade `.deferred` to `.available(payloadText:)` for selected artifact |
| **actionable_state** | Detail preview renders payload text only from server-authorized readback; selection change cancels in-flight detail request |
| **disabled_reason_code** | `disabledReasonCode` from server; maps to `P031DisabledReasonCode` |
| **mutation_availability** | none |
| **cancellation_policy** | `cancel_on_surface_dismiss`, `cancel_on_run_switch`, `cancel_on_selection_change`, `clear_in_flight_on_cancel_or_deadline` |
| **stale_list_detail_behavior** | Detail affordance merges over list affordance for the selected artifact only; stale async responses (detail.artifactID ≠ selectedArtifactID) do not overwrite |
| **unauthorized_behavior** | Unauthorized detail readback returns server auth error or redacted field; Swift does not fall back to local storage or raw filesystem |
| **supported_interactions** | `detail_pane`, `quick_look_when_detail_authorized`, `context_menu` |
| **proof_tests** | `Proposal085Tests/availablePayloadMapsToAvailable`, `mergedAffordanceUsesDetailWhenSelectionMatches`, `staleDetailDoesNotOverwriteAfterSelectionChange`, `nilDetailReturnsListAffordance`, `deferredPayloadDetailProvidesHelpText` |

---

### `report.payload.metadata`

| Field | Value |
|---|---|
| **affordance_id** | `report.payload.metadata` |
| **source_graphql_fields** | report metadata through `artifacts(runId:)`, `payloadAvailabilityState`, `payloadUnavailableReasonCode`, `diagnosticId`, `serverDebugDetail` |
| **local_presentation_state** | `P085ArtifactAffordanceState.PayloadPresentation` — defaults to `.metadataOnly` unless dedicated server-owned payload query exists; `generating` and `payload_deferred` remain distinct from `unavailable` |
| **mutation_availability** | none |
| **staleness_deadline** | Any stuck generating/deferred state needs server-owned deadline proof |
| **unauthorized_behavior** | Same as `artifact.preview.listLabel` — no local fallback |
| **proof_tests** | `Proposal085Tests/metadataOnlyMapsToMetadataOnly` |

---

### `freshness.badge.run`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.run` |
| **source_graphql_fields** | `freshnessState`, `projectionPresent`, `projectionUpdatedAt`, `projectionLag` |
| **local_presentation_state** | `P085FreshnessAffordanceState` — `.live`, `.refreshing`, `.projectionLag`, `.stale`, `.unavailable`, `.unauthorized`, `.unknown(rawValue:)` |
| **actionable_state** | Freshness badge is display-only; `canDrivePayloadAvailability = false`, `canDriveApprovalActionability = false` |
| **mutation_availability** | none |
| **unauthorized_behavior** | `unauthorized` freshness state renders badge only; no payload or mutation inference |
| **proof_tests** | `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly`, `liveFresnessIsStillDiagnosticOnly`, `knownFreshnessValuesRoundTrip`, `unknownFreshnessStateFailsClosed` |

---

### `freshness.badge.stage`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.stage` |
| **source_graphql_fields** | `freshnessState`, `projectionPresent`, `projectionUpdatedAt`, `projectionLag` |
| **local_presentation_state** | Same freshness semantics as run badge, scoped to stage projection fields |
| **actionable_state** | Display-only; `canDrivePayloadAvailability = false`, `canDriveApprovalActionability = false` |
| **mutation_availability** | none |
| **proof_tests** | Covered by `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly` |

---

### `freshness.badge.approval`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.approval` |
| **source_graphql_fields** | `freshnessState`, `projectionUpdatedAt`, `projectionLag`, `availableActions` |
| **local_presentation_state** | `P085FreshnessAffordanceState` with `canDriveApprovalActionability = false`; projection lag is diagnostic context only |
| **actionable_state** | Freshness never determines approval button state; approval actionability derives from durable state + caller policy + typed action availability + mutation authorization |
| **stale_projection_policy** | Projection lag may keep the control visible from last known actionable state, but mutation must be conflict-aware; presenter surfaces lag as diagnostic context |
| **mutation_availability** | none (freshness badge itself has no mutation) |
| **proof_tests** | `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly`, `projectionLagConstraintDetectedCorrectly` |

---

### `freshness.badge.artifact`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.artifact` |
| **source_graphql_fields** | `freshnessState`, `payloadAvailabilityState`, `payloadUnavailableReasonCode` |
| **local_presentation_state** | `P085FreshnessAffordanceState`; freshness never means payload presence — payload availability is read from `payloadAvailabilityState` only |
| **actionable_state** | Display-only; `canDrivePayloadAvailability = false` |
| **mutation_availability** | none |
| **proof_tests** | `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly` |

---

### `approval.resolve.approve`

| Field | Value |
|---|---|
| **affordance_id** | `approval.resolve.approve` |
| **source_graphql_fields** | `approvalInbox`, approval durable state, caller policy readback, `availableActions`, `disabledReasonCode`, `writePathState`, `diagnosticId`, `serverDebugDetail` |
| **local_presentation_state** | `P085ApprovalAffordanceState.approveAvailability` — `.actionable`, `.disabled(reasonCode:helpText:)`, `.hidden` |
| **actionable_state** | Actionable only when durable approval state is pending/requested, caller policy allows approve, `availableActions` contains `"approve"`, `writePathState == .available`, and `approveApproval` is authorized |
| **disabled_reason_code** | `P031DisabledReasonCode` value from server: `WRITE_PATH_NOT_AVAILABLE`, `UNSUPPORTED_ACTION`, `PROJECTION_LAG`, `UNAUTHORIZED`, `AMBIGUOUS_APPROVAL_IDENTITY`, `STALE_READ`, `MANAGED_OUTSIDE_UI` |
| **fallback_text** | Derived from `disabledReasonCode`; maps to stable help text via `P085AffordancePresenter` |
| **mutation_availability** | `approveApproval` |
| **mutation_idempotency** | Duplicate same-result submit returns success or `already_resolved` with current durable decision. Conflicting or non-pending submit returns `state_conflict` / `already_resolved`. Transient transport/server failures are not silently retried by Swift; presenter surfaces `transient_error_retryable` and requires operator re-trigger |
| **stale_projection_policy** | Projection lag may keep control visible from last known actionable state; mutation conflict handling is mandatory; presenter maps conflicts to the same disabled reason used by a fresh non-actionable read model |
| **unauthorized_behavior** | Unauthorized reads return typed GraphQL auth error or redacted fields; Swift never falls back to local storage as truth |
| **supported_interactions** | Approve button, context menu `approve`, keyboard shortcut when actionable |
| **proof_tests** | `Proposal085Tests/approvalIsActionableWhenConditionsMet`, `approvalDisabledWhenWritePathNotAvailable`, `approvalDisabledWhenActionNotInAvailableActions`, `disabledApprovalExposesHelpText`, `projectionLagConstraintDetectedCorrectly`; negative fixture `docs/evidence/rollout-contract/negative/p085-approval-actionability-mismatch.json` |

---

### `approval.resolve.reject`

| Field | Value |
|---|---|
| **affordance_id** | `approval.resolve.reject` |
| **source_graphql_fields** | `approvalInbox`, approval durable state, caller policy readback, `availableActions`, `disabledReasonCode`, `writePathState`, `diagnosticId`, `serverDebugDetail` |
| **local_presentation_state** | `P085ApprovalAffordanceState.rejectAvailability` — same shape as approve |
| **actionable_state** | Same durable state, caller policy, typed availability, and mutation authorization requirements as approve, using `rejectApproval` |
| **mutation_availability** | `rejectApproval` |
| **mutation_idempotency** | Same idempotency, conflict, and transient-error behavior as `approval.resolve.approve` |
| **proof_tests** | `Proposal085Tests/approvalIsActionableWhenConditionsMet`, `approvalDisabledWhenWritePathNotAvailable`; negative fixture `docs/evidence/rollout-contract/negative/p085-approval-stale-double-submit-conflict.json` |

---

### `diagnostic.copy`

| Field | Value |
|---|---|
| **affordance_id** | `diagnostic.copy` |
| **source_graphql_fields** | `diagnosticId`, `serverDebugDetail`, `disabledReasonCode`, `freshnessState` |
| **local_presentation_state** | `P085DiagnosticAffordanceState` — `isAvailable`, `diagnosticID`, `serverDebugDetail`, `copyLabel` |
| **actionable_state** | Available when `diagnosticID` is present and `freshnessState != .unauthorized`; `serverDebugDetail` is included only when authorized by principal class |
| **disabled_reason_code** | `UNAUTHORIZED` when redacted; no `diagnosticID` when unavailable |
| **mutation_availability** | none |
| **unauthorized_behavior** | Auth-error transitions set `isAvailable = false`; cached diagnostic state is invalidated on unauthorized freshness transition |
| **supported_interactions** | `trailing_info_button`, `context_menu`, `detail_pane_copy` |
| **proof_tests** | `Proposal085Tests/diagnosticAffordanceAvailableWithDiagnosticID`, `diagnosticAffordanceUnavailableWithoutDiagnosticID`, `diagnosticAffordanceInvalidatesOnUnauthorized` |

---

### `external.command.placeholder`

| Field | Value |
|---|---|
| **affordance_id** | `external.command.placeholder` |
| **source_graphql_fields** | `writePathState`, `disabledReasonCode`, `diagnosticId` |
| **local_presentation_state** | Hidden or display-only placeholder |
| **actionable_state** | Never actionable from SwiftUI; remains hidden or externally managed |
| **mutation_availability** | `external_transport_only` — start, cancel, retry, reset, compact, clone, recovery, experiment, runtime-profile, and other command actions do not become SwiftUI mutations |
| **proof_tests** | Enforced structurally by P031 mutation boundary; verified by `Proposal031ThinGraphQLReadBoundaryTests/readRequestRejectsMutationDocuments` |

---

## GraphQL Schema Proof Requirements

The `p085` gate asserts SDL/introspection proof for every field and enum domain named by
contract rows, including:

- `payloadAvailabilityState` and its enum domain: `available`, `metadata_only`, `payload_deferred`, `generating`, `unavailable`
- `payloadUnavailableReasonCode` and its enum domain
- `freshnessState` and its enum domain
- `disabledReasonCode` and its enum domain
- `writePathState` and its enum domain
- `diagnosticId`
- `serverDebugDetail`
- `availableActions`
- `approveApproval` mutation
- `rejectApproval` mutation
- Artifact list/detail entrypoints
- Report metadata/payload readback fields

These are proved by existing P031/P043 SDL snapshots and P072 approval mutation fixtures,
composed by the p085 gate slice.

---

## Open Questions Resolution

**Q: Does implementation introduce new persisted projection fields for `stalenessDeadlineAt`
or stalled reason codes?**  
A: No new persisted projection fields are introduced by P085. Stalled/deadline semantics are
server-owned in existing projection shape. The rollout contract migrations section remains
not_applicable.

**Q: Will approval action availability use a new typed GraphQL enum or freeze existing
`availableActions` values?**  
A: Existing `availableActions: [String]` is used. The contract row (`approval.resolve.approve`,
`approval.resolve.reject`) names the string values `"approve"` and `"reject"` as the typed
vocabulary. Unknown action strings are treated as unavailable.

**Q: What exact artifact detail entrypoint carries authorized `payloadText`?**  
A: `P031ArtifactReadModel.payloadText` (from `artifact(id:)` detail entrypoint) carries the
authorized payload. The list entrypoint (`artifacts(runId:)`) may omit `payloadText`; the
detail affordance may upgrade `.deferred` to `.available(payloadText:)` for the selected artifact.

---

## Implementation Notes

- `P085AffordancePresenter` is the canonical Swift mapping layer.
- All presenter outputs are immutable, Equatable, Sendable.
- Unknown GraphQL enum values fail closed at the boundary: `P031FreshnessState`,
  `P031DisabledReasonCode`, `P031WritePathState`, `P031PayloadAvailabilityState`, and
  `P031PayloadUnavailableReasonCode` define a custom `init(from decoder:)` that decodes
  unknown raw strings to a safe sentinel (`.unavailable`, `.writePathNotAvailable`, or
  `.unknown`) rather than throwing. The presenter layer adds `P085FreshnessState.fromRaw(_:)`
  → `.unknown(rawValue:)` and `P085AffordancePresenter.payloadPresentation(fromRaw:)` →
  `.unknown(rawState:)`.
- Approval actionability requires durable decision state to be unresolved:
  `P031ApprovalReadModel.canApprove` and `canReject` evaluate `decision == nil` before
  checking `availableActions` and `writePathState`. `P085AffordancePresenter.approvalAffordance`
  short-circuits to `.disabled` when `decision != nil`, surfacing "Approval is already
  resolved." help text. This guards against stale projection lag presenting an actionable
  button after the durable state has already moved.
- Typed mutation conflict codes are decoded from `P072ApprovalMutationResult.conflictResultCode`
  into `P085MutationConflictResultCode`: `.alreadyResolved`, `.stateConflict`,
  `.transientErrorRetryable`, or `.unknown(rawValue:)`. Unknown server codes are not
  collapsed into success or silently retried.
- Freshness never drives payload availability or approval actionability;
  `P085FreshnessAffordanceState.canDrivePayloadAvailability` and
  `canDriveApprovalActionability` are always `false`.
- Stale async detail responses do not overwrite newer selection:
  `P085AffordancePresenter.mergedAffordance(list:detail:selectedArtifactID:)` returns the
  list affordance when `detail.artifactID ≠ selectedArtifactID`.
- Diagnostic affordance invalidates on `unauthorized` freshness transition:
  `P085DiagnosticAffordanceState.isAvailable = false` when `freshnessState == .unauthorized`.
- Production thin-UI presenters delegate to P085: `P031ApprovalInboxPresenter.rowPresentation`
  derives `canApprove`/`canReject` from `P085AffordancePresenter.approvalAffordance`, and
  `P031ArtifactPresenter.presentation` uses `P085AffordancePresenter.artifactListAffordance`
  for the row's payload-availability label so `payload_deferred` surfaces "Open to preview"
  rather than a generic unavailable fallback.

---

## Gate

```bash
./scripts/test-gate.sh proposal-085
./scripts/test-gate.sh p085
```

Both aliases run the same proof slice (Python contract checks + Swift presenter tests).
