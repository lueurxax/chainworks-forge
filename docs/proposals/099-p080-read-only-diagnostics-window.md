# Proposal 099: P080 Read-Only Diagnostics Window

| Field | Value |
|---|---|
| Date | 2026-06-02 |
| Status | Draft |
| Author | Codex |
| Depends on | P080 Phase 1 detection-only evidence, [080-continuous-stale-execution-reconciliation.md](080-continuous-stale-execution-reconciliation.md), [068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md](068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md), [081-boundary-first-api-auth-contract-matrix.md](081-boundary-first-api-auth-contract-matrix.md) |
| Related | P069 bounded diagnostics UI, P077 closeout readiness UI, P088 completion diagnostics, `docs/reference/rust-control-plane.md` |
| Scope | Add a read-only macOS diagnostics window for P080 stale execution reconciliation after P080 Phase 1 proves useful live readback. |
| Non-goal | No repair, hold, retry, approve, cancel, or daemon-control command from SwiftUI. No implementation before P080 detection-only evidence exists. |

## 1. Problem

P080 creates a precise server-side readback for stale running truth: useful work, startup stale, scheduler ownership drift, helper orphan drift, side-effect drift, and delegated prompt stale. Operators currently inspect this through MCP, reports, logs, or raw diagnostics.

That is acceptable for the first backend rollout, but it is not ideal for day-to-day supervision. Operators need to answer:

- which running rows are useful versus stale;
- which stale rows are held by policy, side-effect safety, rollout phase, or operator hold;
- which class is recurring often enough to justify the next P080 rollout phase;
- whether daemon restarts or retries are safe.

The UI must not become a control surface. Chainworks has a deliberate boundary: SwiftUI reads GraphQL projections and performs approval-only mutations; MCP owns operational commands.

## 2. Evidence Gate Before Implementation

P099 must not enter implementation until P080 Phase 1 detection-only evidence exists.

Minimum required evidence:

- at least one P080 readback projection sample with real nonterminal runs;
- stale class counts for a soak window of at least 48 hours or a shorter operator-approved incident window;
- proof that readback distinguishes useful active work from stale suspected rows;
- proof that GraphQL projection fields are stable enough for a generated client;
- explicit decision that the window will display only classes observed or intentionally documented as empty.

If P080 Phase 1 shows no recurring stale classes and no operator decision benefit, P099 should remain draft or be retired.

## 3. Goals

- Add a native macOS `Window > P080 Diagnostics` read-only window.
- Render the server-owned `p080_readback_v1` projection through GraphQL snapshot plus subscription.
- Make stale/repaired/held/useful states understandable without reading logs.
- Preserve the UI boundary: no MCP calls, no GraphQL mutations, no local repair logic.
- Provide accessibility, lifecycle, and refresh behavior suitable for unattended run supervision.
- Keep visual density operational and table-like, not marketing or card-heavy.

## 4. Non-Goals

- No `Retry`, `Repair`, `Hold`, `Clear Hold`, `Cancel`, `Restart Daemon`, or `Approve` action.
- No local classification of stale truth in Swift.
- No reading SQLite directly from the app.
- No worktree/process inspection from SwiftUI.
- No notification center or menu-bar alert in v1.
- No replacement for MCP diagnostics, run reports, or steward analysis.

## 5. UI Boundary

SwiftUI consumes GraphQL read-only fields only.

Allowed:

- initial GraphQL snapshot query;
- GraphQL subscription for projection updates;
- local filter/sort UI preferences;
- copy-safe diagnostic identifiers;
- links to run reports or reference docs.

Forbidden:

- GraphQL mutation other than existing approval-only mutations unrelated to P080;
- MCP command invocation;
- shell/process execution;
- daemon restart;
- local stale classification used as authority;
- direct DB reads.

Static scans must prove no Swift target invokes P080 MCP mutation/repair/hold tools.

## 6. Window Placement and Lifecycle

Add a menu item:

```text
Window > P080 Diagnostics
```

Window behavior:

- singleton window per app session;
- activation focuses the existing window;
- no default global keyboard shortcut;
- no auto-open on app launch;
- opt out of native window tabbing;
- restore only if visible at last quit and after initial snapshot succeeds;
- snapshot must complete before rows are rendered.

Coordinator:

```text
@MainActor final class P080DiagnosticsWindowCoordinator
```

Responsibilities:

- idempotent activation;
- single snapshot-plus-subscription task;
- cancellation on scene inactivity;
- subscription replacement after reconnect or projection generation change;
- no ad-hoc background tasks outside the owned lifecycle pipeline.

## 7. Data Flow

The window uses:

1. initial GraphQL snapshot;
2. generated strict DTOs;
3. mapping into app view models;
4. subscription events merged only if `projection_updated_at` is newer than the current row;
5. snapshot-then-subscribe refresh after daemon restart, reconnect, wake, app activation, or stale projection heartbeat.

Generated DTOs reject unknown enum values on the v1 channel. App view models may display an unknown state only when the daemon says projection integrity is stale or rebuilt. Transport decode failures do not silently render as unknown.

## 8. Display Model

Compact row fields:

- stale class glyph and label;
- running truth;
- one time field.

Time field precedence:

- if the row is held by cooldown, permanent hold, ambiguous owner, side-effect drift, rollout disablement, or manual operator hold and has `next_retry_or_backoff_time`, show the next retry/backoff time;
- otherwise show elapsed hold/stale age.

Expanded row fields:

- run id;
- stage id;
- work item id;
- agent execution id when present;
- provider/model when present;
- stale class;
- running truth;
- repair action;
- hold reason;
- rollout disablement;
- projection integrity;
- next operator step;
- correlation fields for MCP/report follow-up.

Use dense macOS operational layout: table/list rows with disclosure details. Do not nest cards inside cards.

## 9. Status Vocabulary

Display status from server enums only.

Running truth examples:

- `useful_work_active`;
- `warmup_pending`;
- `stale_suspected`;
- `stale_repaired`;
- `needs_operator`;
- `needs_effect_reconciliation`;
- `unknown`.

Stale class examples:

- `acp_startup_stale`;
- `acp_prompt_stale_delegated`;
- `scheduler_ownership_drift`;
- `helper_orphan_drift`;
- `release_side_effect_drift`;
- `ambiguous_owner`;
- `unknown`.

Every status must have both glyph and label, so it remains understandable without color.

## 10. Refresh Triggers

The window refreshes through snapshot-then-subscribe on:

- GraphQL reconnect;
- daemon version/projection generation change;
- system wake;
- app activation;
- scene becomes active;
- network reachable transition;
- projection heartbeat stale beyond two reconciliation intervals.

Triggers coalesce through a 750 ms debounce window. A new refresh cancels the previous subscription task before opening a new one.

## 11. Accessibility and Localization

Acceptance evidence must cover:

- VoiceOver labels for rows, disclosure controls, filters, and copied identifiers;
- Full Keyboard Access traversal;
- keyboard disclosure activation;
- focus restoration after close/reopen and app relaunch;
- system text-size scaling;
- Differentiate Without Color;
- Reduce Motion;
- sanitized bidi/RTL operator messages;
- unknown enum snapshot tests.

## 12. Empty, Loading, and Error States

Empty state:

```text
No P080 holds or stale rows are active.
```

Show last refresh time and projection integrity in secondary typography.

Loading state:

- show snapshot loading without stale placeholder rows;
- do not render prior rows after projection generation changes until a fresh snapshot arrives.

Error state:

- distinguish daemon unavailable, GraphQL decode failure, permission denied, stale projection, and subscription disconnected;
- do not show a repair action button;
- provide copy-safe correlation fields when available.

## 13. Security and Redaction

The window must never render:

- bearer tokens;
- provider raw transcript text;
- raw prompt content;
- raw provider error strings outside server-redacted fields;
- unredacted operator notes;
- full idempotency keys.

Copy actions use server-redacted diagnostic envelopes.

## 14. Tests and Gates

Add `proposal-099` / `p099` gate coverage:

- generated GraphQL DTO decoding accepts current `p080_readback_v1` and rejects unknown v1 enums;
- snapshot-then-subscribe ordering;
- stale subscription event dropped;
- projection generation change forces fresh snapshot;
- singleton window activation;
- tabbing disabled;
- no SwiftUI P080 mutation/MCP command strings;
- empty/loading/error states;
- accessibility traversal and VoiceOver labels;
- status glyph+label mapping;
- Dynamic Type truncation behavior;
- remote macOS UI smoke for the window.

## 15. Rollout

1. Keep P099 draft while P080 Phase 1 detection-only evidence is absent.
2. After evidence exists, run proposal-readiness review with macOS UI and API-contract reviewers.
3. Implement read-only GraphQL query/subscription client and DTO mapping.
4. Add the diagnostics window behind a UI feature flag.
5. Enable by default only after remote macOS UI evidence and static mutation-boundary scans pass.

## 16. Acceptance Criteria

- Operators can see P080 stale/readback state without raw logs or SQLite inspection.
- The window never performs operational control.
- The UI reflects server truth and does not classify stale rows locally.
- Observed stale classes, holds, rollout disablements, and side-effect blocks are understandable from the row and expanded details.
- Accessibility, refresh, and projection-staleness behavior are covered by tests and remote macOS evidence.
- P099 does not start implementation until P080 Phase 1 evidence proves the window is useful.

