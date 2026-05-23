# Proposal 046: Session Management GraphQL API, Read and Subscription Only

> Source: current unfinished-run proposal artifact.

## Metadata

- **Source run:** `b4f4b41c-b1a1-4d1a-a89b-6d54e1500c0c`
- **Source artifact:** `.chainworks/runs/b4f4b41c-b1a1-4d1a-a89b-6d54e1500c0c/proposals/approved/proposal.md`
- **Source md5:** `96a0a2de0ca4b0c45b13c3d6ecc1250f`
- **Proposal Revision Id:** p046-session-graphql-read-subscription-r4
- **Document Format:** proposal_json_v2
- **Status:** draft_for_review
- **Date:** 2026-05-21
- **Source Run Id:** b4f4b41c-b1a1-4d1a-a89b-6d54e1500c0c

## Summary

Expose run-scoped session lineage, generation, event, KPI, health, and live status observability through GraphQL without adding any GraphQL session reset or control mutation. Reset and reset-agent-session remain MCP-only operational controls. This revision tightens DB retry budgets, subscription backpressure, disabled-schema compatibility, derived sensitive references, connection schema precision, and SwiftUI ownership boundaries.

## Problem

### Current State
- The Rust control plane persists session_lineages, session_generations, session_events, and session execution fields on agent_executions.
- GraphQL exposes some runtime session metadata indirectly, but operators lack a dedicated run-scoped read model for lineage, event history, KPIs, health, and live status changes.
- The corrected P046 scope is observability-only for GraphQL. Session reset and session control remain owned by MCP, not GraphQL or governed SwiftUI controls.
- The feature flag can remove P046 fields from the schema, so clients must not issue P046 documents until capability discovery proves the fields are available.
- **Operator Need:** When a run stalls, reuses stale context, approaches a context budget, resumes with unclear session state, or has ambiguous reset history, the operator needs compact readback that explains what happened without creating a second session-control path.
- **Scope Correction:** P046 implements GraphQL read and subscription surfaces only. It explicitly forbids resetSession and equivalent GraphQL control mutations.

## Goals

- Add GraphQL queries sessionLineages, sessionLineage, sessionGenerations, sessionEvents, sessionKpiSummary, and sessionHealth.
- Add GraphQL subscription sessionStatusChanged(runId) for session lifecycle and status-relevant changes.
- Require resource-scoped operator-read authorization for every query and every subscription emission.
- Bound all potentially large reads with deterministic ordering, default limits, maximum limits, resolver deadlines, opaque cursor semantics, and sanitized invalid-cursor errors.
- Expose stable versioned GraphQL enums and documented non-exhaustive strings where forward compatibility is required.
- Redact event details with a default-deny, versioned, size-bounded allowlist.
- Keep raw provider secrets, prompts, transcripts, hidden reasoning, raw paths, and bearer-like provider session identifiers out of GraphQL.
- Preserve MCP ownership of session reset and control and document that boundary.
- Pin deterministic retry, backpressure, shutdown, lag, and stale-state behavior so observability cannot hang resolvers or exhaust subscription resources.

## Non Goals

- No resetSession GraphQL mutation and no equivalent GraphQL mutation under another name.
- No GraphQL mutation to reset, close, invalidate, compact, retry, recover, cancel, or otherwise control a session.
- No SwiftUI reset button, governed SwiftUI MCP reset bridge, or Swift-local reset fallback added by P046.
- No change to MCP reset command semantics.
- No new authentication, refresh-token, RBAC, credential UI, or user provisioning work.
- No SQL schema migration or index migration in P046. If implementation requires an index, this proposal must be revised before implementation freeze.
- No raw prompt, transcript, hidden reasoning, provider credential, bearer-like provider session identifier, absolute working directory, or unredacted diagnostics exposure through GraphQL.
- No SwiftData persistence of P046 readback state in this proposal. P046 client state is transient MainActor UI state unless a later proposal extends local projections.

## Ux Ui Notes

### Allowed Ui Behavior
- Show session lineages for a run grouped by agent and reuse scope.
- Show generation status, active marker, provider/model, bounded safe references, turn count, token/cost counters, last activity, and end reason.
- Show health warnings inline using closed message templates keyed by reasonCode.
- Show reset recommendations as guidance text only, for example: Suggested MCP action: use the MCP session reset capability.
- Subscribe to sessionStatusChanged to refresh visible read models after status changes, only after feature/capability discovery proves the subscription exists.
### Forbidden Ui Behavior
- Do not render a reset button wired to GraphQL.
- Do not add a governed SwiftUI control that invokes MCP reset as part of P046.
- Do not add GraphQL mutations for reset, compact, retry, cancel, recover, invalidate, close, or any other non-approval command.
- Do not display raw providerSessionId, raw bindingFingerprint, raw invocationOwnerKey, or absolute workingDirectory.
- Do not issue P046 GraphQL documents against a disabled schema.
### Presentation Guidance
- Use existing run/stage diagnostic surfaces and compact status rows.
- Health warnings should be inline and non-modal.
- Fields absent on older runs render as unknown or not recorded.
- When capability discovery says P046 is disabled or fields are absent, hide the session observability panel or show the existing generic unavailable state without logging GraphQL validation errors as product errors.
### Swiftui Ownership
- Introduce a dedicated MainActor P046SessionObservabilityModel owned by the selected-run detail coordinator. Existing run detail views may observe this model, but they do not own GraphQL tasks or subscription lifetimes.
- Use .task(id: runId) or equivalent selected-run scoped task ownership so changing selected run cancels the previous query/subscription before starting a new one.
- Capability discovery or schema introspection runs before constructing P046 query/subscription documents. Disabled-schema tests must prove no P046 document is sent when the feature is unavailable.
- On subscription close, lag, reconnect, shutdown, slow_consumer, or resyncRequired=true, mark the model stale and re-query sessionLineages, sessionKpiSummary, and sessionHealth before resuming live rendering.
- Deduplicate events by eventId, then lineageId/generationId/recordedAt as fallback.
- Decode unknown enum values and nullable historical fields as unknown/not recorded, not as view failures.
- P046 readback state is transient MainActor UI state. It is not written to SwiftData or treated as local durable truth in this proposal.
- If an AppKit bridge is touched, it is presentation-only: it may host SwiftUI or forward presentation events, but it must not own GraphQL tasks, reset affordances, SwiftData writes, or session cache mutation.

## Architecture

### Crate Boundaries
- domain owns stable session enums and persisted data shape already used by db and engine.
- db owns SQL read helpers, cursor pagination helpers, aggregate queries, bounded retry implementation for safe transient sqlite failures, cancellation propagation to sqlx, and row parsing. It does not own GraphQL redaction decisions.
- graphql-server owns GraphQL naming, authorization checks, sensitivity replacements, redaction, resolver deadlines, subscription projection, bounded per-subscriber buffers, slow-consumer policy, disabled-schema behavior, and schema snapshots.
- engine remains the owner of session lifecycle writes and event emission.
- mcp-server remains the owner of non-approval operational control. P046 does not add or rename MCP reset tools.
- SwiftUI consumes P046 as transient readback only through the existing GraphQL transport/store boundary and a single selected-run session observability owner.
- **Data Model:** P046 uses existing session_lineages, session_generations, session_events, and agent_executions session columns. No SQL migration or additive index is allowed in this slice. If profiling proves an index is needed, implementation must stop and revise the proposal and rollout_contract_v1 before freeze.
### Failure Behavior
- Missing run or unauthorized run returns the existing GraphQL authorization/not-found behavior without leaking session rows.
- Malformed IDs return sanitized GraphQL parse errors consistent with existing behavior and without echoing raw input.
- Cross-lineage generationId filters return not-found-or-not-visible behavior.
- Corrupt timestamps or invalid enum strings produce resolver errors and, where health can be computed, bounded critical warnings.
- Subscription lag, overflow, reconnect, graceful shutdown, or daemon restart never mutates state; clients re-query before rendering current state.
- Transient authorization lookup failure during subscription emission terminates the stream fail-closed rather than emitting stale data.
- Transient sqlite busy/timeout cannot spin beyond the retry budget or resolver deadline.
### Implementation Slices
1. **Item**
   - **Details**
     - Add list_lineages_for_run_paginated, find_lineage_owner_run, list_generations_for_lineage_paginated, find_generation_with_lineage, list_events_paginated, aggregate_kpis_for_run, and health helper queries.
     - Use deterministic ordering and opaque cursor inputs for list helpers.
     - Implement the P046 transient sqlite retry policy:3 total attempts,50ms then150ms backoff, jitter capped at25ms, retry sleep budget capped at250ms, and stop before the2 second resolver deadline loses250ms of headroom.
     - Propagate cancellation through async-graphql deadlines to sqlx queries and stop retries after cancellation.
     - Do not add write paths for P046.
   - **Slice:** DB read helpers
2. **Item**
   - **Details**
     - Add connection DTOs, Edge, PageInfo, and session DTOs following existing async-graphql patterns.
     - Call resource-scoped authorization before returning any data.
     - Replace raw sensitive generation fields with derived safe references that are non-secret, non-reversible, and scoped.
     - Use resolver deadlines:2 seconds for queries and250 milliseconds for subscription payload resolution.
     - Add schema snapshot tests for exact additions, connection shape, cursor nullability, disabled-schema behavior, and forbidden reset/control mutations.
   - **Slice:** GraphQL DTOs and resolvers
3. **Item**
   - **Details**
     - Use existing event sender/BroadcastStream patterns with a bounded per-subscriber queue of64 payloads.
     - Filter by run_id before resolving details.
     - Recheck authorization on every emission and terminate fail-closed on revocation or transient authorization lookup failure.
     - Emit at most one resyncRequired=true payload between successful non-resync payloads after lag or overflow.
     - Disconnect slow consumers when the queue remains full for5 seconds or3 consecutive enqueue attempts fail.
     - Payloads carry eventId, lineageId, generationId, recordedAt, and status for deduplication.
   - **Slice:** Subscription event bridge
4. **Item**
   - **Details**
     - Compute health from persisted session rows, not provider live handles.
     - Use p046_session_health_thresholds_v1 defaults.
     - Zero session rows produce state=UNKNOWN with reasonCode=no_session_data.
     - Transient DB busy/timeout produces state=UNKNOWN with reasonCode=transient_db_unavailable after bounded retry; sustained data-shape corruption can escalate to CRITICAL.
   - **Slice:** Health and KPI projection
5. **Item**
   - **Details**
     - Document GraphQL observability and MCP-only reset ownership.
     - Add SwiftUI guardrail proving no resetSession GraphQL document or governed reset UI/action path is introduced by P046.
     - Add client capability/schema discovery so P046 GraphQL documents are not issued when fields are absent under the feature flag.
     - Keep MCP reset guidance generic until implemented MCP tool names are confirmed.
     - Add proposal-046 gate coverage for rollout contract, schema, authorization, pagination, redaction, subscription filtering, revoked authorization, slow consumers, retry exhaustion, disabled schema, and negative fixtures.
   - **Slice:** Docs, UI, and guardrails

## Graphql Contract

### Authorization
- **General:** Every P046 query and subscription requires the existing GraphQL operator-read policy. Missing principals return unauthorized. Principals without operator-read return forbidden or not-found-or-not-visible according to resolver shape.
- **Resource Visibility Rules**
  - sessionLineages(runId) parses runId, applies operator-read authorization for that run, then reads lineages. Unauthorized callers receive forbidden and no DB session rows are returned.
  - sessionLineage(id) first resolves the lineage id to its owning run using a minimal lookup, applies operator-read authorization for that run, then returns the lineage. If the lineage is absent or not visible, return null without revealing which case occurred.
  - sessionGenerations(lineageId) first resolves lineageId to its owning run, applies operator-read authorization for that run, then returns generations. If absent or not visible, return an empty connection with uniform not-found-or-not-visible behavior.
  - sessionEvents(lineageId,generationId) first resolves lineageId to its owning run, applies operator-read authorization for that run, then verifies generationId belongs to that lineage before returning events. A generationId from another lineage returns the same not-found-or-not-visible shape as an absent generation.
  - sessionKpiSummary(runId) and sessionHealth(runId) apply operator-read authorization for runId before aggregate reads.
  - Malformed IDs use existing GraphQL ID parse errors but must not echo raw input or disclose row existence to unauthorized callers.
- **Subscription Lifecycle**
  - sessionStatusChanged(runId) authorizes the principal for runId during subscription setup.
  - Before yielding every event, the resolver rechecks operator-read authorization for runId. If authorization is revoked, downgraded, expired, unavailable, or the authorization lookup returns a transient error, the stream terminates fail-closed with a sanitized authorization error and emits no further payloads.
  - The resolver filters by run_id before resolving details and before yielding a payload.
  - A negative test must prove a revoked principal stops receiving events.
### Connection Schema
- **Connection Fields**
  - edges: [Edge!]!
  - nodes: [Node!]!
  - pageInfo: PageInfo!
- **Cursor Nullability:** Edge cursors are non-null. startCursor and endCursor are null only when the returned page is empty.
- **Edge Fields**
  - cursor: String!
  - node: Node!
- **Has Next Page Semantics:** Resolvers fetch limit+1 rows after applying authorization and filters. hasNextPage is true only when an extra row exists beyond the returned page.
- **Invalid Cursor Error:** Malformed, expired, wrong-type, or mismatched-filter cursors return a sanitized GraphQL BAD_USER_INPUT-style error with message 'invalid cursor' and no raw cursor echo.
- **Page Info Fields**
  - hasNextPage: Boolean!
  - startCursor: String
  - endCursor: String
- **Snapshot Requirement:** SDL snapshots must cover Connection, Edge, PageInfo, cursor nullability, hasNextPage semantics, and invalid-cursor fixture behavior for every P046 connection.
### Field Sensitivity
- **Bindingfingerprint**
  - **Classification:** derived-replacement-required
  - **Derived Reference Rule:** bindingProfileRef is a non-secret, non-reversible reference scoped to the owning run or control-plane instance and tested not to equal or contain the raw binding fingerprint.
  - **Proposal Field:** bindingProfileRef: ID
  - **Rationale:** Raw fingerprints can reveal or correlate provider configuration. Expose only a control-plane issued non-secret reference.
- **Invocationownerkey**
  - **Classification:** derived-replacement-required
  - **Derived Reference Rule:** invocationOwnerRef is a non-secret, non-reversible reference scoped to the owning run and must not contain raw owner keys, stage ids, task ids, or agent execution ids unless those ids are separately classified safe for operator readback.
  - **Proposal Field:** invocationOwnerKind: SessionInvocationOwnerKind! and invocationOwnerRef: ID
  - **Rationale:** Raw owner keys can leak workflow ownership internals. Expose bounded owner kind plus a non-secret derived reference.
- **Providersessionid**
  - **Classification:** derived-replacement-required
  - **Derived Reference Rule:** providerSessionRef is a non-secret, non-reversible, control-plane-issued reference scoped to the owning run and local control-plane instance. It must not preserve provider id prefixes, lengths, encodings, or stable cross-run correlation.
  - **Proposal Field:** hasProviderSession: Boolean! and providerSessionRef: ID
  - **Rationale:** Provider session ids may be bearer-like or correlatable across provider adapters. GraphQL must not expose the raw provider id.
- **Workingdirectory**
  - **Classification:** derived-replacement-required
  - **Derived Reference Rule:** workingDirectoryDisplay may be repo-relative inside the workspace or a closed redaction string such as '<outside-workspace redacted>'. It must never contain an absolute path, home directory, username, or provider temp directory.
  - **Proposal Field:** workspaceMode: String! and workingDirectoryDisplay: String
  - **Rationale:** Absolute paths leak local filesystem layout. Expose workspace mode and repo-relative or redacted display text only.
### Forbidden Schema
- Mutation.resetSession
- Mutation.resetAgentSession
- Mutation.sessionsReset
- Mutation.closeSession
- Mutation.invalidateSession
- Mutation.compactSession
- Mutation.retrySession
- Mutation.recoverSession
- Mutation.cancelSession
- Any GraphQL mutation that closes, resets, invalidates, compacts, retries, recovers, cancels, or otherwise controls a session.
### Queries
1. **sessionLineages**
   - **Bounds:** Ordered by agent_id, lineage_id, created_at, id. Default first=100, max first=500. Cursor is opaque base64 over the stable sort tuple.
   - **Name:** sessionLineages
   - **Signature:** sessionLineages(runId: ID!, first: Int =100, after: String): SessionLineageConnection!
2. **sessionLineage**
   - **Bounds:** Single-row lookup after parent-run authorization.
   - **Name:** sessionLineage
   - **Signature:** sessionLineage(id: ID!): SessionLineage
3. **sessionGenerations**
   - **Bounds:** Ordered by generation ascending, created_at, id. Default first=100, max first=500. Cursor is opaque base64 over the stable sort tuple.
   - **Name:** sessionGenerations
   - **Signature:** sessionGenerations(lineageId: ID!, first: Int =100, after: String): SessionGenerationConnection!
4. **sessionEvents**
   - **Bounds:** Ordered by recorded_at ascending, id ascending. Default first=200, max first=1000. Cursor is opaque base64 over recorded_at and id. Requests above max fail validation before DB access.
   - **Name:** sessionEvents
   - **Signature:** sessionEvents(lineageId: ID!, generationId: ID, first: Int =200, after: String): SessionEventConnection!
5. **sessionKpiSummary**
   - **Bounds:** Server-side aggregate with a2 second resolver deadline and cancellation propagation.
   - **Name:** sessionKpiSummary
   - **Signature:** sessionKpiSummary(runId: ID!): SessionKpiSummary!
6. **sessionHealth**
   - **Bounds:** Server-side persisted-row projection with a2 second resolver deadline and cancellation propagation.
   - **Name:** sessionHealth
   - **Signature:** sessionHealth(runId: ID!): SessionHealthReport!
### Redaction Contract
- **Allowed Top Level Keys**
  - schemaVersion
  - summaryCode
  - providerKind
  - modelFamily
  - reuseDisposition
  - resetReason
  - endReason
  - tokenEstimateBucket
  - contextWindowPressureBucket
  - checkpointPresent
  - repairAttemptCount
  - safeDiagnosticCode
- **Forbidden Content**
  - raw prompt text
  - transcript text
  - hidden reasoning
  - provider credentials
  - bearer tokens
  - absolute filesystem paths
  - raw provider diagnostics
  - raw command output
  - unbounded free-form error strings
- **Graphql Shape:** detailsJsonRedacted: SessionEventDetailsRedacted
- **Max Serialized Bytes:** `4096`
- **Message Safety:** SessionHealthWarning.message is selected from closed templates keyed by reasonCode. It must not interpolate raw ids, paths, provider errors, prompts, or diagnostics.
- **Ownership:** graphql-server owns the redaction allowlist as a versioned constant and tests it with negative fixtures. db returns persisted JSON only to the resolver; db does not decide operator safety.
- **Partial Safe Behavior:** If an event contains both allowed and disallowed keys, the resolver returns only typed safe fields and omits detailsJsonRedacted unless the entire object conforms to the allowlist and size limit.
- **Schema Version:** p046_event_details_redaction_v1
- **Unknown Behavior:** Unknown event_type, unknown details schema_version, unknown top-level key, malformed JSON, or serialized output over4096 bytes returns detailsJsonRedacted=null and appends a bounded warning reason code. Unknown shapes never pass raw JSON through GraphQL.
### Subscription
- **Backpressure Policy**
  - **Overflow Behavior:** When the per-subscriber queue is full or broadcast lag is detected, drop live history for that subscriber, try to enqueue one resyncRequired=true payload, and suppress further resyncRequired payloads until a successful non-resync payload is delivered.
  - **Per Subscriber Buffer Size:** `64`
  - **Resource Bound:** No resolver may allocate unbounded per-subscriber buffers or retain dropped event payloads after resyncRequired is scheduled.
  - **Slow Consumer Disconnect:** If the subscriber queue remains full for5 seconds, or if3 consecutive enqueue attempts fail for the same subscriber, terminate the subscription with a sanitized slow_consumer error and no buffered replay.
  - **Test Requirement:** Tests must cover overflow, at-most-once resyncRequired between successful payloads, slow-consumer disconnect, and re-query requirement.
- **Delivery Contract**
  - Payloads are small and do not include event history.
  - Each payload includes eventId, lineageId, generationId, recordedAt, and status so clients can deduplicate and detect gaps.
  - Delivery is best-effort live notification, not durable event replay. Clients must re-query sessionLineages, sessionKpiSummary, and sessionHealth after reconnect, lag, daemon restart, graceful shutdown, or resyncRequired=true.
  - On broadcast lag or overflow, the server emits at most one resyncRequired=true payload between successful non-resync payloads. The client must then re-query before treating state as current.
  - If the daemon restarts or the WebSocket closes, SwiftUI treats the view as stale until fresh query readback completes.
- **Name:** sessionStatusChanged
- **Shutdown And Lag**
  - **Emit Lag Slo:** During dogfood, p95 session_status_subscription_emit_lag_seconds must be below500 milliseconds and p99 below2 seconds for local operator runs. Breach is a rollout hold until diagnosed or explicitly waived.
  - **Event Id Invariant:** eventId is unique and stable for each persisted session_events row across daemon restarts. If provider input lacks a safe event id, derive eventId from the control-plane event row identity, not from raw provider identifiers.
  - **Graceful Shutdown:** On daemon graceful shutdown, attempt to send one resyncRequired=true payload with eventType=UNKNOWN_EVENT_SHAPE and status=RESYNC_REQUIRED when possible, allow up to1 second of drain, then close the stream. Failure to send during shutdown is acceptable because reconnect requires re-query.
  - **Out Of Order Events:** sessionEvents pagination is deterministic over recorded_at,id. Retroactive inserts with a sort tuple before a previously consumed cursor are not guaranteed to appear in forward-only pagination; clients must perform a full re-query after resyncRequired, reconnect, daemon restart, or detected cursor gap.
- **Signature:** sessionStatusChanged(runId: ID!): SessionStatusChangedEvent!
### Type Sketch
- **Sessionevent**
  - id
  - lineageId
  - generationId
  - eventId
  - eventType
  - recordedAt
  - typedDetails
  - detailsJsonRedacted
  - redactionWarnings
- **Sessiongeneration**
  - id
  - lineageId
  - generation
  - hasProviderSession
  - providerSessionRef
  - bindingProfileRef
  - invocationOwnerKind
  - invocationOwnerRef
  - workspaceMode
  - workingDirectoryDisplay
  - runtimeProvider
  - runtimeModel
  - status
  - turnCount
  - estimatedInputTokens
  - latestCachedInputTokens
  - latestOutputTokens
  - latestModelContextWindow
  - cumulativePromptTokens
  - cumulativeCostCents
  - createdAt
  - lastActivityAt
  - endedAt
  - endReason
  - isActive
- **Sessionhealthreport**
  - runId
  - state
  - warnings
  - checkedAt
  - thresholdsVersion
- **Sessionhealthwarning**
  - reasonCode
  - severity
  - lineageId
  - generationId
  - message
  - suggestedMcpAction
- **Sessionkpisummary**
  - runId
  - lineageCount
  - generationCount
  - activeGenerationCount
  - closedGenerationCount
  - resetGenerationCount
  - invalidatedGenerationCount
  - reuseEventCount
  - operatorResetEventCount
  - totalTurnCount
  - totalPromptTokens
  - totalCostCents
  - latestActivityAt
  - staleActiveGenerationCount
- **Sessionlineage**
  - id
  - runId
  - agentId
  - lineageKey
  - sessionReuseScope
  - sessionFamilyId
  - activeGenerationId
  - createdAt
  - closedAt
  - activeGeneration
  - generationCount
  - latestEventAt
  - healthState
- **Sessionstatuschangedevent**
  - runId
  - lineageId
  - generationId
  - eventId
  - status
  - eventType
  - recordedAt
  - healthState
  - resyncRequired
### Vocabulary
- **Compatibility:** GraphQL enum values are stable for v1. New reason codes are non-exhaustive strings under p046_session_graphql_vocab_v1 and must use bounded snake_case values. Swift clients must decode unknown enum/status values defensively into unknown(String) or equivalent fallback display.
- **Graphql Enums**
  - **Sessionendreason**
    - COMPLETED
    - FAILED
    - OPERATOR_RESET
    - INVALIDATED
    - CONTEXT_PRESSURE
    - TRANSPORT_ERROR
    - TIMEOUT
    - UNKNOWN
  - **Sessioneventtype**
    - LINEAGE_CREATED
    - GENERATION_STARTED
    - SESSION_REUSED
    - GENERATION_CLOSED
    - GENERATION_INVALIDATED
    - OPERATOR_RESET_RECORDED
    - CHECKPOINT_REHYDRATED
    - REPAIR_ATTEMPTED
    - REPAIR_FAILED
    - CONTEXT_WINDOW_OBSERVED
    - UNKNOWN_EVENT_SHAPE
  - **Sessiongenerationstatus**
    - ACTIVE
    - CLOSED
    - INVALIDATED
    - RESET
    - FAILED
    - UNKNOWN
  - **Sessionhealthseverity**
    - INFO
    - WARNING
    - CRITICAL
  - **Sessionhealthstate**
    - HEALTHY
    - WARNING
    - CRITICAL
    - UNKNOWN
  - **Sessionstatuschangedstatus**
    - ACTIVE
    - CLOSED
    - INVALIDATED
    - RESET
    - FAILED
    - UNKNOWN
    - RESYNC_REQUIRED
- **Reason Codes**
  - stale_active_generation
  - active_generation_missing
  - generation_without_lineage
  - repeated_operator_reset
  - invalidated_active_generation
  - context_window_pressure
  - repair_failure_recent
  - no_session_data
  - transient_db_unavailable
  - redaction_unknown_event_type
  - redaction_unknown_schema_version
  - redaction_unknown_details_shape
  - redaction_size_limit_exceeded
  - subscription_resync_required
  - slow_consumer_disconnected
  - authorization_recheck_failed
  - sqlite_retry_exhausted
- **Schema Version:** p046_session_graphql_vocab_v1

## Rollout Contract V1

- **Applicability:** required
### Commands
- **Allowlist**
  - ./scripts/test-gate.sh proposal-046
  - ./scripts/test-gate.sh fast
- **Commentary:** Gate commands are declarative expectations; the rollout linter must not execute them.
### Decision Vocabulary
- pass
- fail
- waived
- not_applicable
- timeout
### Gate Aliases
- proposal-046
### Hold Conditions
- GraphQL schema exposes resetSession or any equivalent session reset/control mutation.
- Any P046 ID-based resolver returns data before resolving parent run ownership and applying operator-read authorization.
- sessionEvents or sessionLineages can return unbounded results.
- Connection, Edge, PageInfo, cursor nullability, hasNextPage, or invalid-cursor behavior is absent from SDL snapshots.
- sessionStatusChanged does not filter by run_id before emitting.
- sessionStatusChanged continues emitting after principal operator-read authorization is revoked or authorization recheck fails transiently.
- sessionStatusChanged lacks a64 payload per-subscriber buffer cap, at-most-once resyncRequired behavior, or slow-consumer disconnect criteria.
- Transient sqlite busy/timeout retry can exceed3 total attempts, the50ms/150ms backoff schedule,250ms retry sleep budget, or the2 second resolver deadline.
- P046 GraphQL reads omit operator authorization checks.
- Session event details expose unredacted prompt, credential, hidden reasoning, raw provider diagnostics, or absolute paths.
- Unknown event_type, schema_version, or details_json shape does not fail closed to detailsJsonRedacted=null plus bounded warning.
- Raw providerSessionId, bindingFingerprint, invocationOwnerKey, absolute workingDirectory, or reversible/scopeless derived references appear in P046 GraphQL schema or payload.
- SwiftUI or another governed client sends P046 documents when schema/capability discovery says P046 fields are absent.
- AppKit owns P046 GraphQL tasks, reset affordances, SwiftData writes, or session cache mutation.
- P046 readback is persisted to SwiftData without a separate persistence proposal.
- KPI, health, retry, subscription, or disabled-schema metrics use unbounded labels such as run_id, lineage_id, generation_id, raw cursor, raw client id, or raw error text.
- Rollout readback fixture is missing, placeholder-only, stale, or outside docs/evidence/rollout-contract.
- proposal-046 gate fails.
### Hold Conditions Detail
- Any GraphQL reset/control mutation violates the corrected P046 boundary and blocks rollout.
- Opaque IDs must not become cross-run information disclosure channels.
- Large reads must be bounded before SwiftUI consumption.
- Connection schema precision must be frozen before clients depend on it.
- Subscriptions must be run-scoped to avoid cross-run information disclosure.
- Long-lived subscriptions must respect authorization changes and fail closed on lookup uncertainty.
- Subscription backpressure must remain bounded under stalled clients.
- SQLite contention must not consume the resolver deadline or produce nondeterministic health.
- Read surfaces must preserve the existing operator-read authorization policy.
- Details JSON must be redacted or omitted unless the shape is proven operator-safe.
- Redaction defaults to deny for unknown event and JSON shapes.
- Sensitive provider/session identifiers must be replaced with scoped non-reversible references.
- Feature-flagged schema removal must not cause client-side validation noise or broken UI.
- AppKit is presentation-only if touched by this feature.
- P046 does not create local durable session projections.
- Metrics must remain bounded-cardinality.
- Rollout fixtures must live under docs/evidence/rollout-contract and become proposal-specific evidence before release.
- The proposal-specific gate is the release proving path.
### Metrics
- **Adoption Metric:** session_graphql_observability_query_success_rate
- **Operational Metrics**
  - session_graphql_query_total{field,status}
  - session_graphql_query_duration_seconds{field}
  - session_graphql_sqlite_retry_total{field,outcome}
  - session_graphql_sqlite_retry_exhausted_total{field}
  - session_status_subscription_event_total{event_type,status}
  - session_status_subscription_emit_lag_seconds{event_type}
  - session_status_subscription_slow_consumer_disconnect_total{reason}
  - session_health_warning_total{reason_code,severity}
  - session_graphql_reset_mutation_guard_total{status}
  - session_event_redaction_total{reason_code,status}
  - session_graphql_disabled_schema_guard_total{client,status}
### Migrations
- **Justification:** P046 is restricted to GraphQL read/subscription resolvers over existing session persistence. No SQL schema migration or additive index is allowed in this proposal revision.
- **Not Applicable:** `true`
### Negative Fixtures
- **Appkit Owns Graphql Task:** docs/evidence/rollout-contract/negative/p046-appkit-owns-graphql-task.json
- **Authorization Recheck Transient Open:** docs/evidence/rollout-contract/negative/p046-authorization-recheck-transient-open.json
- **Disabled Schema Client Unguarded:** docs/evidence/rollout-contract/negative/p046-disabled-schema-client-unguarded.json
- **Imprecise Connection Schema:** docs/evidence/rollout-contract/negative/p046-imprecise-connection-schema.graphql
- **Missing Parent Run Authorization:** docs/evidence/rollout-contract/negative/p046-missing-parent-run-authorization.json
- **Missing Run Filter Subscription:** docs/evidence/rollout-contract/negative/p046-missing-run-filter-subscription.json
- **Raw Sensitive Generation Fields:** docs/evidence/rollout-contract/negative/p046-raw-sensitive-generation-fields.graphql
- **Reset Mutation Present:** docs/evidence/rollout-contract/negative/p046-reset-mutation-present.graphql
- **Resync Churn:** docs/evidence/rollout-contract/negative/p046-resync-churn.json
- **Reversible Derived Reference:** docs/evidence/rollout-contract/negative/p046-reversible-derived-reference.json
- **Revoked Subscription Principal:** docs/evidence/rollout-contract/negative/p046-revoked-subscription-principal.json
- **Slow Consumer No Disconnect:** docs/evidence/rollout-contract/negative/p046-slow-consumer-no-disconnect.json
- **Swiftdata Persistence Leak:** docs/evidence/rollout-contract/negative/p046-swiftdata-persistence-leak.json
- **Unbounded Metric Labels:** docs/evidence/rollout-contract/negative/p046-unbounded-metric-labels.json
- **Unbounded Session Events:** docs/evidence/rollout-contract/negative/p046-unbounded-session-events.graphql
- **Unbounded Sqlite Retry:** docs/evidence/rollout-contract/negative/p046-unbounded-sqlite-retry.json
- **Unknown Event Type Redaction:** docs/evidence/rollout-contract/negative/p046-unknown-event-type-redaction.json
- **Unredacted Event Details:** docs/evidence/rollout-contract/negative/p046-unredacted-event-details.json
### Operator Report Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
### Readback Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- **Readback Fixture:** docs/evidence/rollout-contract/operator-readback/p046-session-graphql-full-surface.fixture.json
### Readback Lanes
- run_report
- mcp
- release_receipt
- graphql
### Rollback Disposition
- **Data Loss Risk:** none
- **Mode:** feature_flag_disable_graphql_session_observability_fields
- **Steps**
  - Set CHAINWORKS_GRAPHQL_SESSION_OBSERVABILITY to disabled or remove P046 schema registration in the rollback patch.
  - Leave existing session persistence untouched because P046 owns no lifecycle writes.
  - Keep MCP reset/control behavior unchanged.
  - Keep clients capability-gated so disabled schema returns to the existing unavailable state.
  - Publish release receipt noting GraphQL session observability disabled and MCP reset ownership unaffected.
- **Schema Version:** rollout_contract_v1

## Rollout

### Feature Flag
- **Client Compatibility:** SwiftUI and other governed clients must gate P046 documents on schema introspection, capability discovery, or a known enabled dogfood configuration. Disabled-schema tests must prove clients do not send P046 documents and present a non-error unavailable state.
- **Default:** disabled until the proposal-046 gate passes; enabled for dogfood by explicit config; release default may flip only after dogfood success is recorded
- **Identifier:** CHAINWORKS_GRAPHQL_SESSION_OBSERVABILITY
- **Owning Crate:** control-plane/crates/graphql-server
- **Runtime Source:** daemon environment/config loaded before schema construction
- **Schema Registration Seam:** Schema construction conditionally attaches P046 query fields and subscription only when the flag is enabled. When disabled, fields are absent from the schema and existing GraphQL behavior is unchanged.
### Phases
1. **Item**
   - **Exit Criteria**
     - rollout_contract_v1 passes lint or engine materializes declared missing fixture placeholders under docs/evidence/rollout-contract before approved-proposal preflight.
     - proposal-046 gate alias exists and is documented.
     - Schema snapshot test proves reset/control mutations are absent.
     - P046-specific negative fixtures are non-empty and fail the intended checks before release evidence is accepted.
     - Retry budget, slow-consumer policy, disabled-schema client behavior, and connection SDL fixtures exist.
   - **Phase:** Phase0: Contract and guardrails
2. **Item**
   - **Exit Criteria**
     - All six GraphQL query fields resolve from seeded SQLite fixtures.
     - Resource-scoped authorization tests cover runId and ID-based resolvers.
     - Pagination defaults, maximums, cursors, ordering, PageInfo fields, and invalid-cursor validation are tested.
     - Transient sqlite busy/timeout fixtures prove max attempts, backoff, deadline headroom, cancellation propagation, and deterministic exhaustion behavior.
     - Sensitive SessionGeneration raw fields are absent from GraphQL and derived references are proven non-secret and non-reversible.
   - **Phase:** Phase1: Bounded read queries
3. **Item**
   - **Exit Criteria**
     - sessionStatusChanged emits only for matching run_id.
     - Revoked principals and transient authorization lookup failures stop subscription emissions.
     - Broadcast lag produces at-most-once resyncRequired behavior and client re-query guidance.
     - Per-subscriber buffer limit, slow-consumer disconnect, graceful shutdown drain, out-of-order cursor behavior, eventId stability, and emit-lag SLO are tested.
     - Health thresholds cover stale, missing, reset, invalidated, context pressure, transient DB unavailable, and no-data cases.
     - Metrics emit bounded labels only.
     - Redaction unknown-shape fixtures return null plus bounded warnings.
   - **Phase:** Phase2: Subscription, health, metrics, and redaction
4. **Item**
   - **Exit Criteria**
     - P046SessionObservabilityModel is MainActor-scoped and owned by the selected-run detail coordinator.
     - SwiftUI gates P046 documents on capability/schema availability and handles disabled-schema mode without GraphQL validation errors.
     - SwiftUI displays readback and generic reset guidance only.
     - No SwiftUI reset control, AppKit reset bridge, SwiftData persistence, or GraphQL reset mutation path exists.
     - Reference docs document GraphQL observability, transient UI state, disabled-schema behavior, and MCP-only reset ownership.
     - Dogfood validation meets success metrics.
   - **Phase:** Phase3: SwiftUI readback and docs
- **Rollback Procedure:** Disable CHAINWORKS_GRAPHQL_SESSION_OBSERVABILITY or remove P046 schema registration in a rollback patch. Do not alter existing session tables, engine lifecycle writes, or MCP reset/control paths.
### Rollback Triggers
- Any reset/control mutation appears in GraphQL schema.
- Operator authorization regression on P046 fields.
- Cross-run subscription leak.
- Subscription continues after authorization revocation or transient authorization lookup failure.
- Unredacted sensitive event details in GraphQL.
- Unbounded sessionEvents or sessionLineages reads reach implementation.
- Transient sqlite retry loop can exceed the pinned retry budget or resolver deadline.
- Subscription per-subscriber buffering exceeds64 queued payloads or slow consumers are not disconnected.
- P046 clients issue GraphQL documents when schema fields are disabled or absent.
- P046 metrics use unbounded labels.
- proposal-046 or fast gate fails because of P046.

## Metrics

### Bounded Labels
- **Client**
  - swiftui
  - graphql_external
- **Event Type**
  - LINEAGE_CREATED
  - GENERATION_STARTED
  - SESSION_REUSED
  - GENERATION_CLOSED
  - GENERATION_INVALIDATED
  - OPERATOR_RESET_RECORDED
  - CHECKPOINT_REHYDRATED
  - REPAIR_ATTEMPTED
  - REPAIR_FAILED
  - CONTEXT_WINDOW_OBSERVED
  - UNKNOWN_EVENT_SHAPE
- **Field**
  - sessionLineages
  - sessionLineage
  - sessionGenerations
  - sessionEvents
  - sessionKpiSummary
  - sessionHealth
- **Outcome**
  - success_after_retry
  - exhausted
  - cancelled
  - deadline_headroom_stop
- **Reason**
  - queue_full_5s
  - consecutive_enqueue_failures
  - shutdown_drain_timeout
- **Reason Code**
  - stale_active_generation
  - active_generation_missing
  - generation_without_lineage
  - repeated_operator_reset
  - invalidated_active_generation
  - context_window_pressure
  - repair_failure_recent
  - no_session_data
  - transient_db_unavailable
  - redaction_unknown_event_type
  - redaction_unknown_schema_version
  - redaction_unknown_details_shape
  - redaction_size_limit_exceeded
  - subscription_resync_required
  - slow_consumer_disconnected
  - authorization_recheck_failed
  - sqlite_retry_exhausted
- **Severity**
  - INFO
  - WARNING
  - CRITICAL
- **Status**
  - ok
  - unauthorized
  - forbidden
  - not_found
  - invalid_argument
  - timeout
  - db_unavailable
  - redacted
  - disabled_schema
  - slow_consumer
  - error
### Dogfood Validation
- **Alerts And Runbooks**
  - Alert if session_graphql_reset_mutation_guard_total{status='fail'} is greater than0. Owner: control-plane on-call. Runbook: docs/reference/runbooks/p046-session-graphql.md.
  - Alert if session_graphql_observability_query_success_rate drops below0.95 in the dogfood window. Owner: control-plane on-call. Runbook: docs/reference/runbooks/p046-session-graphql.md.
  - Alert if p95 session_status_subscription_emit_lag_seconds exceeds500 milliseconds or p99 exceeds2 seconds during dogfood. Owner: control-plane on-call. Runbook: docs/reference/runbooks/p046-session-graphql.md.
  - Alert if session_graphql_sqlite_retry_exhausted_total is non-zero outside injected tests during dogfood. Owner: control-plane on-call. Runbook: docs/reference/runbooks/p046-session-graphql.md.
- **Cohort:** local operator daemon using the control-plane GraphQL endpoint during the P046 dogfood build window.
- **Cross Run Check:** For every sessionStatusChanged emit trace, assert emitted run_id equals subscribed run_id; any mismatch is a rollout hold.
- **Trace Source:** structured control-plane logs and GraphQL metrics emitted by the dogfood daemon.
- **Window:** One working day after Phase2 gate pass or at least20 runs with session data, whichever comes first.
### Operational Metrics
- session_graphql_query_total{field,status}
- session_graphql_query_duration_seconds{field}
- session_graphql_sqlite_retry_total{field,outcome}
- session_graphql_sqlite_retry_exhausted_total{field}
- session_status_subscription_event_total{event_type,status}
- session_status_subscription_emit_lag_seconds{event_type}
- session_status_subscription_lag_total{status}
- session_status_subscription_slow_consumer_disconnect_total{reason}
- session_health_warning_total{reason_code,severity}
- session_graphql_reset_mutation_guard_total{status}
- session_event_redaction_total{reason_code,status}
- session_graphql_disabled_schema_guard_total{client,status}
### Success Metrics
- At least95 percent of P046 GraphQL observability queries in the dogfood window return status ok for runs with session data.
- Zero GraphQL schema entries for session reset/control mutations.
- Zero cross-run sessionStatusChanged emissions in tests and dogfood traces.
- Session health warnings include bounded reason codes for every injected fixture failure case.
- No P046 resolver exceeds its2 second deadline under injected sqlite busy/timeout fixtures; health exhaustion returns UNKNOWN/transient_db_unavailable deterministically.
- No subscription allocates more than64 queued payloads per subscriber; slow consumers disconnect according to policy.
- Dogfood p95 session_status_subscription_emit_lag_seconds is below500 milliseconds and p99 is below2 seconds.

## Health Thresholds

- **Configurability:** Thresholds are fixed for P046 and documented by thresholdsVersion. Operator-configurable thresholds are deferred until dogfood data justifies them.
### Defaults
- **Active Generation Missing:** lineage.active_generation_id is non-null but the generation row cannot be found.
- **Context Window Pressure:** latestCachedInputTokens plus latestOutputTokens is at least85 percent of latestModelContextWindow, or estimatedInputTokens is at least85 percent when latest fields are absent.
- **Generation Without Lineage:** A generation row references a missing lineage.
- **Invalidated Active Generation:** lineage.active_generation_id points to a generation whose status is INVALIDATED or RESET.
- **No Session Data:** No session_lineages rows exist for the authorized run. Health state is UNKNOWN with reasonCode=no_session_data; HEALTHY with no warnings is not legal for zero session rows.
- **Repair Failure Recent:** At least2 repair failure events for the same lineage within30 minutes.
- **Repeated Operator Reset:** At least2 operator reset events for the same lineage within30 minutes, or at least3 within24 hours.
- **Stale Active Generation:** Active generation with no lastActivityAt update for more than15 minutes while the run is still non-terminal.
- **Transient Db Unavailable:** Transient sqlite busy or timeout after bounded retry. Health state is UNKNOWN with reasonCode=transient_db_unavailable.
- **Schema Version:** p046_session_health_thresholds_v1

## Acceptance Criteria

- GraphQL exposes the corrected read fields and sessionStatusChanged subscription when CHAINWORKS_GRAPHQL_SESSION_OBSERVABILITY is enabled.
- GraphQL exposes no resetSession mutation and no equivalent session reset or control mutation.
- All P046 resolvers require operator-read authorization scoped to the owning run.
- ID-based resolvers resolve parent lineage or generation ownership before returning data and use not-found-or-not-visible behavior for unauthorized rows.
- sessionStatusChanged filters by run_id, rechecks authorization for every emission, and stops emitting after authorization revocation or transient authorization lookup failure.
- sessionEvents and other large reads enforce default limits, maximum limits, deterministic ordering, cursor behavior, and connection SDL snapshots.
- Session KPIs and health are computed server-side from persisted session data.
- Event details are redacted or omitted by a default-deny, versioned, tested operator-safe rule.
- ProviderSessionId, bindingFingerprint, invocationOwnerKey, and workingDirectory are replaced with derived operator-safe fields rather than exposed raw.
- Transient sqlite busy/timeout uses a pinned retry policy that cannot exceed the2 second resolver deadline and returns deterministic db_unavailable or UNKNOWN/transient_db_unavailable on exhaustion.
- sessionStatusChanged uses a bounded per-subscriber buffer, slow-consumer disconnect policy, and at-most-once resyncRequired emission between successful payloads.
- Docs state reset and reset-agent-session remain MCP-only.
- SwiftUI does not expose GraphQL session reset controls, gates P046 documents on schema/capability availability, and keeps P046 state MainActor-owned.
- rollout_contract_v1 is present, uses repo-allowed rollout fixture paths, and is covered by the proposal gate.

## Test Plan

- GraphQL schema snapshot tests for all new fields and absence of reset/control mutations.
- SDL snapshot tests for Connection, Edge, PageInfo, cursor nullability, hasNextPage, and sanitized invalid-cursor behavior.
- DB repo tests for lineage/generation/event pagination, parent-run lookup, generation-lineage membership validation, KPI aggregation, and health fixtures.
- Transient sqlite busy/timeout tests proving3 total attempts,50ms/150ms backoff, retry sleep cap, deadline headroom, cancellation propagation, and deterministic exhaustion outputs.
- Authorization tests for runId resolvers, ID-based resolvers, subscription setup, per-emission revocation, and transient authorization lookup failure.
- Subscription test publishing events for two runs and proving only the requested run emits.
- Broadcast lag/reconnect/overflow tests proving at-most-once resyncRequired between successful payloads, required re-query contract,64-payload buffer cap,5-second full-queue disconnect, and3 consecutive enqueue failure disconnect.
- Graceful shutdown test proving attempted drain then close and stale/re-query client handling.
- Out-of-order event insert test documenting cursor behavior and full re-query after resync.
- eventId stability test across daemon restart and client dedupe fixture.
- Redaction tests for safe, partial-safe, unsafe, unknown event_type, unknown schema_version, unknown keys, malformed JSON, and oversized payloads.
- Sensitivity tests proving raw providerSessionId, bindingFingerprint, invocationOwnerKey, absolute workingDirectory, and reversible derived references are absent from GraphQL.
- Health tests for no_session_data, stale_active_generation, active_generation_missing, generation_without_lineage, invalidated_active_generation, repeated_operator_reset, context_window_pressure, repair_failure_recent, and transient_db_unavailable.
- SwiftUI guardrail proving no resetSession GraphQL document, no governed reset UI/action path, no SwiftData persistence, and no AppKit-owned GraphQL task is introduced by P046.
- Disabled-schema client test proving P046 documents are gated by capability/schema discovery and not sent when fields are absent.
- Rollout fixture tests proving negative fixtures fail closed and operator readback fields are populated.

## Risks

1. **Item**
   - **Mitigation:** Add schema snapshot and negative fixture proving reset/control mutations are absent.
   - **Risk:** GraphQL schema accidentally reintroduces session reset/control because old P046 text mentioned resetSession.
2. **Item**
   - **Mitigation:** Resolve parent run first, authorize against that run, validate generation-lineage membership, and use not-found-or-not-visible behavior.
   - **Risk:** Opaque IDs in ID-based resolvers leak metadata across runs.
3. **Item**
   - **Mitigation:** Enforce pagination, maximum limits, deterministic cursors, resolver deadlines, cancellation propagation, and query latency telemetry.
   - **Risk:** Long-running sessions create expensive event reads or aggregate queries.
4. **Item**
   - **Mitigation:** Use the pinned retry budget:3 total attempts,50ms/150ms backoff,250ms retry sleep cap,250ms resolver headroom, and deterministic db_unavailable or UNKNOWN/transient_db_unavailable on exhaustion.
   - **Risk:** SQLite contention causes resolver hangs or inconsistent health behavior.
5. **Item**
   - **Mitigation:** Use a64 payload per-subscriber buffer, at-most-once resyncRequired between successful payloads, and disconnect after5 seconds full or3 consecutive enqueue failures.
   - **Risk:** Slow WebSocket consumers consume unbounded memory or churn resync notifications.
6. **Item**
   - **Mitigation:** Use default-deny typed redaction with schema version, allowlisted keys, size limit, and unknown-shape warnings.
   - **Risk:** Session event details_json contains provider diagnostics or prompt fragments.
7. **Item**
   - **Mitigation:** Recheck authorization at every emission, terminate on revocation or transient lookup failure, emit bounded resyncRequired on lag where possible, and require client re-query after reconnect or shutdown.
   - **Risk:** Long-lived subscriptions leak after principal revocation or silently drift stale after lag/restart.
8. **Item**
   - **Mitigation:** Replace providerSessionId, bindingFingerprint, invocationOwnerKey, and workingDirectory with non-secret, non-reversible, scoped references or redacted display fields and add negative fixtures against raw values.
   - **Risk:** Sensitive provider/session identifiers are exposed raw or through reversible derived references.
9. **Item**
   - **Mitigation:** Require schema/capability gating before issuing P046 documents and disabled-schema tests for SwiftUI and governed clients.
   - **Risk:** Clients fail noisily when the feature flag removes P046 fields from the schema.
10. **Item**
   - **Mitigation:** Phase0 and rollout hold conditions require proposal-specific fixture content before the proposal gate is accepted as release evidence.
   - **Risk:** Declared rollout fixtures remain placeholder-only after preflight materialization.

## Proposal Feedback Coverage

### Backlog Items Addressed
- SLB-P046-R3-001
- SLB-P046-R3-002
- SLB-P046-R3-003
- SLB-P046-R3-004
- SLB-P046-R3-005
- SLB-P046-R3-006
- SLB-P046-R3-007
- SLB-P046-R3-008
- SLB-P046-R3-009
### Backlog Items Deferred
_None._
### Backlog Items Disputed
_None._
### Backlog Items Unresolved
_None._
### Factual Claims Added Or Corrected
- Pinned transient sqlite busy/timeout retry policy:3 total attempts,50ms and150ms backoff, jitter capped at25ms, retry sleep budget capped at250ms, per-attempt busy timeout capped at300ms, and250ms resolver deadline headroom.
- Pinned deterministic retry exhaustion behavior: sessionHealth returns UNKNOWN/transient_db_unavailable and other P046 queries return sanitized db_unavailable without partial data.
- Pinned subscription backpressure:64 payload per-subscriber buffer, at-most-once resyncRequired between successful payloads, disconnect after5 seconds full or3 consecutive enqueue failures.
- Added disabled-schema client compatibility: SwiftUI and governed clients must gate P046 documents on capability/schema discovery or known enabled dogfood configuration.
- Defined derived sensitive references as non-secret, non-reversible, scoped to the owning run or local control-plane instance, and tested against raw sensitive values.
- Pinned Connection, Edge, PageInfo, cursor nullability, hasNextPage semantics, and sanitized invalid-cursor error behavior for SDL snapshots.
- Named P046SessionObservabilityModel as a dedicated MainActor model owned by the selected-run detail coordinator.
- Declared AppKit presentation-only if touched and prohibited AppKit ownership of GraphQL tasks, reset affordances, SwiftData writes, or session cache mutation.
- Clarified P046 readback is transient MainActor UI state and not persisted to SwiftData in this proposal.
- Added graceful shutdown drain, transient auth recheck fail-closed behavior, subscription emit-lag SLO, out-of-order cursor behavior, sqlx cancellation propagation, and eventId uniqueness/stability invariants.
- **Notes:** The current score_lift_backlog review_pass_id is authoritative. This revision addresses only SLB-P046-R3-001 through SLB-P046-R3-009 from review pass983ef6b8-c54d-4218-af39-ff74f340a953-review-pass-1 and does not carry forward stale blocker mappings from earlier review passes.
- **Proposal Revision Id:** p046-session-graphql-read-subscription-r4
### Sections Changed
- problem
- goals
- non_goals
- acceptance_criteria
- graphql_contract
- reliability_contract
- architecture
- health_thresholds
- ux_ui_notes
- metrics
- rollout
- risks
- open_questions
- test_plan
- rollout_contract_v1
- **Source Review Pass Id:** 983ef6b8-c54d-4218-af39-ff74f340a953-review-pass-1

## Open Questions

1. **Item**
   - **Question:** What exact MCP reset tool names should UI guidance display?
   - **Recommended Default:** Keep UI and docs generic: Suggested MCP action: use the MCP session reset capability. Do not publish a concrete tool name until implemented MCP docs/schema confirm it.
2. **Item**
   - **Question:** Should sessionHealth thresholds be configurable?
   - **Recommended Default:** No for P046. Ship fixed p046_session_health_thresholds_v1 defaults and revisit after dogfood data.
3. **Item**
   - **Question:** Should sessionEvents expose redacted JSON or only typed detail fields?
   - **Recommended Default:** Expose typed safe detail fields for known event types. Keep detailsJsonRedacted nullable and default-deny for all unknown or unproven shapes.
4. **Item**
   - **Question:** Should P046 add SQL indexes if dogfood finds slow queries?
   - **Recommended Default:** No in this revision. Stop implementation and revise the proposal plus rollout contract if an index migration becomes necessary.
5. **Item**
   - **Question:** Should disabled-schema capability discovery use introspection or a dedicated capability field?
   - **Recommended Default:** Use whichever existing GraphQL capability/introspection pattern is already present in the codebase. If neither exists, add the smallest read-only capability query in graphql-server and cover it with disabled-schema tests.

## Reliability Contract

### Resolver Deadlines
- **Cancellation Propagation:** async-graphql deadlines must cancel downstream futures. P046 db helpers must not continue retry loops after resolver cancellation, and sqlx query futures must be dropped promptly when the deadline fires.
- **Query Deadline Ms:** `2000`
- **Subscription Payload Resolution Deadline Ms:** `250`
### Transient Sqlite Retry Policy
- **Applies To**
  - sqlite busy
  - sqlite timeout
  - transient pool acquisition timeout where existing repo policy classifies retry as safe
- **Backoff Ms**
  - `50`
  - `150`
- **Deadline Headroom Ms Min:** `250`
- **Exhaustion Behavior:** If all attempts fail or remaining resolver budget would fall below deadline_headroom_ms_min, stop retrying. sessionHealth returns state=UNKNOWN with reasonCode=transient_db_unavailable and a bounded warning. Other P046 queries return a sanitized db_unavailable resolver error without partial data.
- **Jitter Ms Max:** `25`
- **Max Attempts Total:** `3`
- **Metrics**
  - session_graphql_sqlite_retry_total{field,outcome}
  - session_graphql_sqlite_retry_exhausted_total{field}
- **Per Attempt Busy Timeout Ms Max:** `300`
- **Retry Sleep Budget Ms Max:** `250`
- **Test Requirement:** Fixtures must prove max attempts, backoff cap, deadline headroom, cancellation propagation, and deterministic UNKNOWN/transient_db_unavailable on health exhaustion.
