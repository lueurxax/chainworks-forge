# Thin-Client Read-Model Affordance Contract

**Contract schema:** `thin_client_affordance_contract_v1`  
**Status:** Implemented  
**Contract version:** `p085-r2-2026-05-08`

**Retained gate/test alias:** `proposal-085` / `p085` / `P085`

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
| Removed or retyped fields | Breaking change — requires explicit contract revision, gate update, and migration-contract update if persisted projection shape changes |
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
| **mutation_idempotency** | not_applicable — list label exposes no mutation |
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
| **fallback_text** | `.deferred` -> "Open the artifact to fetch an authorized preview"; `.unavailable` -> server-owned unavailable reason; `.unknown` -> unknown-state diagnostic |
| **mutation_availability** | none |
| **mutation_idempotency** | not_applicable — detail preview exposes no mutation |
| **staleness_deadline** | Current backend-emitted deferred states include an explicit no-deadline justification in `serverDebugDetail`; `generating` must not be emitted by this row unless the server also emits a deadline or stalled diagnostic |
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
| **actionable_state** | Read-only metadata row; `canOpenPayload = false` unless the server explicitly returns `payloadAvailabilityState = available` from an authorized payload query |
| **disabled_reason_code** | not_applicable — read-only row; payload unavailability is represented by `payloadUnavailableReasonCode` |
| **fallback_text** | `.metadataOnly` -> "metadata only"; `.payloadDeferred` -> "Open to preview" only when a detail query can authorize payload; `.unavailable` -> "Unavailable" |
| **mutation_availability** | none |
| **mutation_idempotency** | not_applicable — no mutation is available from this row |
| **staleness_deadline** | Current backend-emitted metadata/deferred states include an explicit no-deadline justification in `serverDebugDetail`; `generating` must not be emitted by this row unless the server also emits a deadline or stalled diagnostic |
| **cancellation_policy** | not_applicable for list metadata; any future detail payload request must cancel on surface dismiss, run switch, or selection change |
| **stale_list_detail_behavior** | Metadata row remains list-owned; detail may only upgrade payload state for the currently selected artifact/report |
| **unauthorized_behavior** | Same as `artifact.preview.listLabel` — no local fallback |
| **supported_interactions** | `list_row`, `detail_pane_metadata`, `diagnostic_copy_when_authorized` |
| **proof_tests** | `Proposal085Tests/metadataOnlyMapsToMetadataOnly` |

---

### `freshness.badge.run`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.run` |
| **source_graphql_fields** | `freshnessState`, `projectionPresent`, `projectionUpdatedAt`, `projectionLag` |
| **local_presentation_state** | `P085FreshnessAffordanceState` — `.live`, `.refreshing`, `.projectionLag`, `.stale`, `.unavailable`, `.unauthorized`, `.unknown(rawValue:)` |
| **actionable_state** | Freshness badge is display-only; `canDrivePayloadAvailability = false`, `canDriveApprovalActionability = false` |
| **disabled_reason_code** | not_applicable — freshness never disables or enables controls by itself |
| **fallback_text** | Unknown future freshness values render as "Unknown" diagnostic state |
| **mutation_availability** | none |
| **mutation_idempotency** | not_applicable — freshness badge is read-only |
| **staleness_deadline** | not_applicable — badge reports projection recency; it does not own payload-generation deadlines |
| **cancellation_policy** | refresh/subscription reads may be cancelled on run switch or surface dismiss |
| **stale_list_detail_behavior** | Run-level freshness is not merged with artifact/detail payload state |
| **unauthorized_behavior** | `unauthorized` freshness state renders badge only; no payload or mutation inference |
| **supported_interactions** | `runs_home_row`, `run_detail_header`, `freshness_badge` |
| **proof_tests** | `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly`, `liveFresnessIsStillDiagnosticOnly`, `knownFreshnessValuesRoundTrip`, `unknownFreshnessStateFailsClosed` |

---

### `freshness.badge.stage`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.stage` |
| **source_graphql_fields** | `freshnessState`, `projectionPresent`, `projectionUpdatedAt`, `projectionLag` |
| **local_presentation_state** | Same freshness semantics as run badge, scoped to stage projection fields |
| **actionable_state** | Display-only; `canDrivePayloadAvailability = false`, `canDriveApprovalActionability = false` |
| **disabled_reason_code** | not_applicable — stage freshness does not own actionability |
| **fallback_text** | Unknown future freshness values render as "Unknown" diagnostic state |
| **mutation_availability** | none |
| **mutation_idempotency** | not_applicable — freshness badge is read-only |
| **staleness_deadline** | not_applicable — stage freshness reports projection recency only |
| **cancellation_policy** | stage projection reads may be cancelled on run switch or surface dismiss |
| **stale_list_detail_behavior** | Stage freshness does not overwrite stage status or payload availability |
| **unauthorized_behavior** | unauthorized freshness is diagnostic-only and cannot authorize local reads |
| **supported_interactions** | `stage_row`, `stage_transition_map`, `freshness_badge` |
| **proof_tests** | Covered by `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly` |

---

### `freshness.badge.approval`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.approval` |
| **source_graphql_fields** | `freshnessState`, `projectionUpdatedAt`, `projectionLag`, `availableActions` |
| **local_presentation_state** | `P085FreshnessAffordanceState` with `canDriveApprovalActionability = false`; projection lag is diagnostic context only |
| **actionable_state** | Freshness never determines approval button state; approval actionability derives from durable state + caller policy + typed action availability + mutation authorization |
| **disabled_reason_code** | not_applicable — approval disabled reason comes from approval row fields, not freshness |
| **fallback_text** | Unknown future freshness values render as "Unknown" diagnostic state |
| **stale_list_detail_behavior** | Projection lag may keep the control visible from last known actionable state, but mutation must be conflict-aware; presenter surfaces lag as diagnostic context |
| **mutation_availability** | none (freshness badge itself has no mutation) |
| **mutation_idempotency** | not_applicable — freshness badge is read-only |
| **staleness_deadline** | not_applicable — approval mutation conflict handling owns stale-submit safety |
| **cancellation_policy** | approval projection reads may be cancelled on run switch or surface dismiss |
| **unauthorized_behavior** | unauthorized freshness is diagnostic-only and cannot authorize local mutation fallback |
| **supported_interactions** | `approval_inbox_row`, `approval_detail`, `freshness_badge` |
| **proof_tests** | `Proposal085Tests/freshnessAffordanceIsAlwaysDiagnosticOnly`, `projectionLagConstraintDetectedCorrectly` |

---

### `freshness.badge.artifact`

| Field | Value |
|---|---|
| **affordance_id** | `freshness.badge.artifact` |
| **source_graphql_fields** | `freshnessState`, `payloadAvailabilityState`, `payloadUnavailableReasonCode` |
| **local_presentation_state** | `P085FreshnessAffordanceState`; freshness never means payload presence — payload availability is read from `payloadAvailabilityState` only |
| **actionable_state** | Display-only; `canDrivePayloadAvailability = false` |
| **disabled_reason_code** | not_applicable — payload disabled reason comes from payload fields, not freshness |
| **fallback_text** | Unknown future freshness values render as "Unknown" diagnostic state |
| **mutation_availability** | none |
| **mutation_idempotency** | not_applicable — freshness badge is read-only |
| **staleness_deadline** | not_applicable — artifact payload deadlines are represented by payload fields and diagnostics |
| **cancellation_policy** | artifact projection/detail reads may be cancelled on run switch, surface dismiss, or selection change |
| **stale_list_detail_behavior** | Freshness badge never upgrades or downgrades list/detail payload availability |
| **unauthorized_behavior** | unauthorized freshness invalidates diagnostics and does not authorize local payload fallback |
| **supported_interactions** | `artifact_list_row`, `artifact_detail`, `freshness_badge` |
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
| **mutation_idempotency** | Duplicate, stale, conflicting, or non-pending submit returns success or `already_resolved` with current durable decision when the backend can journal the attempt. Transient transport/server failures stay GraphQL errors and are not silently retried by Swift; unknown future conflict codes fail closed to `.unknown(rawValue:)` |
| **staleness_deadline** | not_applicable — approval mutation conflict handling owns stale-submit safety |
| **cancellation_policy** | mutation request may be cancelled by caller; server outcome remains command-journal truth |
| **stale_list_detail_behavior** | Projection lag may keep control visible from last known actionable state; mutation conflict handling is mandatory; presenter maps conflicts to the same disabled reason used by a fresh non-actionable read model |
| **unauthorized_behavior** | Unauthorized reads return typed GraphQL auth error or redacted fields; Swift never falls back to local storage as truth |
| **supported_interactions** | Approve button, context menu `approve`, keyboard shortcut when actionable |
| **proof_tests** | `Proposal085Tests/approvalIsActionableWhenConditionsMet`, `approvalDisabledWhenWritePathNotAvailable`, `approvalDisabledWhenActionNotInAvailableActions`, `disabledApprovalExposesHelpText`, `projectionLagConstraintDetectedCorrectly`; negative fixture `docs/evidence/rollout-contract/negative/p085-approval-actionability-mismatch.json` |
| **boundary_matrix_rows** | `P081-UI-APPROVAL-APPROVE`, `P081-UI-READ-ONLY`, `P081-UI-EXTERNAL-COMMANDS` from `docs/reference/ui-action-boundary.md` |

---

### `approval.resolve.reject`

| Field | Value |
|---|---|
| **affordance_id** | `approval.resolve.reject` |
| **source_graphql_fields** | `approvalInbox`, approval durable state, caller policy readback, `availableActions`, `disabledReasonCode`, `writePathState`, `diagnosticId`, `serverDebugDetail` |
| **local_presentation_state** | `P085ApprovalAffordanceState.rejectAvailability` — same shape as approve |
| **actionable_state** | Same durable state, caller policy, typed availability, and mutation authorization requirements as approve, using `rejectApproval` |
| **disabled_reason_code** | Same `P031DisabledReasonCode` vocabulary as approve |
| **fallback_text** | Derived from `disabledReasonCode`; maps to stable help text via `P085AffordancePresenter` |
| **mutation_availability** | `rejectApproval` |
| **mutation_idempotency** | Same idempotency, conflict, and transient-error behavior as `approval.resolve.approve` |
| **staleness_deadline** | not_applicable — approval mutation conflict handling owns stale-submit safety |
| **cancellation_policy** | mutation request may be cancelled by caller; server outcome remains command-journal truth |
| **stale_list_detail_behavior** | Same projection-lag and stale-submit handling as approve |
| **unauthorized_behavior** | Unauthorized reads return typed GraphQL auth error or redacted fields; Swift never falls back to local storage as truth |
| **supported_interactions** | Reject button, context menu `reject`, keyboard shortcut when actionable |
| **proof_tests** | `Proposal085Tests/approvalIsActionableWhenConditionsMet`, `approvalDisabledWhenWritePathNotAvailable`; negative fixture `docs/evidence/rollout-contract/negative/p085-approval-stale-double-submit-conflict.json` |
| **boundary_matrix_rows** | `P081-UI-APPROVAL-REJECT`, `P081-UI-READ-ONLY`, `P081-UI-EXTERNAL-COMMANDS` from `docs/reference/ui-action-boundary.md` |

---

### `diagnostic.copy`

| Field | Value |
|---|---|
| **affordance_id** | `diagnostic.copy` |
| **source_graphql_fields** | `diagnosticId`, `serverDebugDetail`, `disabledReasonCode`, `freshnessState` |
| **local_presentation_state** | `P085DiagnosticAffordanceState` — `isAvailable`, `diagnosticID`, `serverDebugDetail`, `copyLabel` |
| **actionable_state** | Available when `diagnosticID` is present and `freshnessState != .unauthorized`; `serverDebugDetail` is included only when authorized by principal class |
| **disabled_reason_code** | `UNAUTHORIZED` when redacted; no `diagnosticID` when unavailable |
| **fallback_text** | "No Diagnostic Available" when no authorized diagnostic ID is present |
| **mutation_availability** | none |
| **mutation_idempotency** | not_applicable — copy action has no server mutation |
| **staleness_deadline** | not_applicable — diagnostic copy is read-only projection metadata |
| **cancellation_policy** | diagnostic read requests may be cancelled on surface dismiss or run switch |
| **stale_list_detail_behavior** | unauthorized freshness invalidates cached diagnostic affordance rather than merging stale detail |
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
| **disabled_reason_code** | `MANAGED_OUTSIDE_UI` or `WRITE_PATH_NOT_AVAILABLE` when surfaced as guidance |
| **fallback_text** | "Managed outside UI" / "Use MCP or CLI" guidance only; no SwiftUI command button |
| **mutation_availability** | `external_transport_only` — start, cancel, retry, reset, compact, clone, recovery, experiment, runtime-profile, and other command actions do not become SwiftUI mutations |
| **mutation_idempotency** | not_applicable in SwiftUI; MCP/control-plane owns command idempotency |
| **staleness_deadline** | not_applicable — placeholder is guidance, not a running server operation |
| **cancellation_policy** | not_applicable — no SwiftUI command request is started |
| **stale_list_detail_behavior** | placeholder state is derived from current read model and never from local workflow truth |
| **unauthorized_behavior** | unauthorized users see no local fallback or hidden command surface |
| **supported_interactions** | `read_only_guidance`, `disabled_placeholder`, `external_transport_documentation` |
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
- `conflictResultCode` (typed `GqlMutationConflictResultCode` enum on
  `ApproveApprovalPayload` / `RejectApprovalPayload`: `already_resolved`)
- Artifact list/detail entrypoints
- Report metadata/payload readback fields

These are proved by existing P031/P043 SDL snapshots and P072 approval mutation fixtures
plus the P085 backend slice (`cargo test -p graphql-server --lib proposal_085_`), which
covers approval/report projection fields, authorization denial for diagnostic fields, and
typed `conflictResultCode` readback with a real failed `command_journal` id.

---

## Contract Decisions

The implemented contract does not add persisted projection fields for `stalenessDeadlineAt`
or stalled reason codes. Stalled/deadline semantics are server-owned in the existing
projection shape, and the rollout contract migrations section remains `not_applicable`.

Approval action availability uses existing `availableActions: [String]`. The approval
contract rows (`approval.resolve.approve`, `approval.resolve.reject`) define `"approve"`
and `"reject"` as the recognized action vocabulary. Unknown action strings are treated as
unavailable.

`P031ArtifactReadModel.payloadText` from the `artifact(id:)` detail entrypoint carries
authorized payload text. The list entrypoint (`artifacts(runId:)`) may omit `payloadText`;
the detail affordance may upgrade `.deferred` to `.available(payloadText:)` for the
selected artifact.

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
- Approval actionability requires the durable decision state to be unresolved:
  `P031ApprovalReadModel.canApprove` and `canReject` evaluate `isActionableDecision` —
  true when `decision` is `nil`, `"pending"`, or `"requested"` — before checking
  `availableActions` and `writePathState`. `P085AffordancePresenter.approvalAffordance`
  fails closed first on the caller-policy denial codes `unauthorized`, `staleRead`, and
  `ambiguousApprovalIdentity`; then short-circuits to `.disabled` for any other non-nil
  decision (`granted`, `rejected`, `expired`, …), surfacing "Approval is already
  resolved." help text; and only then consults `writePathState`/`availableActions`. This
  guards against stale projection lag presenting an actionable button after the durable
  state has already moved.
- Typed mutation conflict codes are decoded from `P072ApprovalMutationResult.conflictResultCode`
  into `P085MutationConflictResultCode`: `.alreadyResolved` or `.unknown(rawValue:)`.
  Unknown server codes are not collapsed into success or silently retried.
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
