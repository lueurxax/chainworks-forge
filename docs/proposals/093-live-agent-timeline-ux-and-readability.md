# Proposal 093: Live Agent Timeline UX and Readability

Status: Draft
Date: 2026-05-21
Owner: Chainworks Forge
Related proposals: P031, P036
Related references:
- `docs/reference/macos-operator-navigation.md`
- `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md`
- `docs/reference/design-system-and-brand-application.md`
- `docs/brand/README.md`

## Summary

The Timeline tab should become an operator-grade live transcript surface for the
currently selected active agent. It should read like a bounded, focused exchange:
operator prompts, provider responses, tool or command activity, and session
events shown as stable cards. The newest card appears first. At most one card is
expanded at a time. Collapsed cards stay compact and readable; expanded cards
show a bounded formatted preview and expose the full raw detail through copy and
control-plane readback.

This proposal keeps the control plane as the source of truth. It does not bring
back Swift-local orchestration state, and it does not turn Timeline into a stage
history duplicate.

## Problem

The current live Timeline proves the basic data path, but the operator
experience is still not good enough:

- Agent response chunks can feel like they disappear instead of accumulating into
  one readable response.
- Completed responses show raw tail content in the collapsed card, which is noisy
  and often not useful.
- Prompt and response content is hard to read because markdown fences, JSON, and
  dense command output are rendered as plain text.
- Some cards truncate important detail without a consistent way to expand them.
- State, timestamp, and identifiers consume visual space even when the operator
  does not need them immediately.
- One run can have more than one active agent, but the Timeline currently behaves
  like there is only one obvious active source.
- The card order does not match the desired live-operations workflow where the
  newest event is visible first.
- Large provider output must not make the UI slow, jumpy, or unresponsive.

## Goals

1. Show every `Agent Response` as a standard-size card by default.
2. Let a click expand an `Agent Response` card to show a bounded formatted
   preview and make the full raw message copyable.
3. Render readable formatted content, including fenced code blocks and JSON.
4. After an agent response completes, show only general response information in
   the collapsed card, not the message tail.
5. Make the same expand/collapse behavior work for all Timeline event cards,
   including `Prompt sent`.
6. Keep at most one expanded card at a time. Clicking the expanded card collapses
   it; clicking a different card collapses the previous card and expands the new
   one.
7. Show cards in reverse chronological order: newest first, oldest last.
8. Keep the UI responsive with bounded rendering, lazy formatting, and stable row
   identity.
9. Hide state and time until hover or keyboard focus, while keeping event IDs easy
   to copy.
10. Always show agent identity and provider identity on the card.
11. Provide a pleasant active-agent selector when multiple agents are running in
   the same run. If only one active agent exists, select it automatically.

## Non-Goals

- Do not recreate the old Swift-local orchestrator timeline.
- Do not duplicate the Stages tab or stage-history readback.
- Do not store provider text as trusted engine truth beyond existing persisted
  runtime evidence.
- Do not implement full transcript search in this proposal.
- Do not render unbounded provider output directly in collapsed cards.
- Do not use a web view or provider-authored HTML for formatting.

## Current Baseline

`docs/reference/macos-operator-navigation.md` defines Timeline as a control-plane
active-agent readback surface. It is not synthesized from stage transitions,
artifacts, approvals, or Swift-side orchestration state. The existing P036 buffer
coalesces response chunks into a live row and keeps the visible list bounded.

The brand references require Forge-native cards, restrained status treatment,
SF Symbols where icons are needed, accessible status labels, and design tokens
from the app's Forge design system. Timeline should preserve those constraints:
status truth outranks decoration, IDs should use monospaced treatment, and large
text should be readable without dominating the run detail surface.

## Proposed Experience

### Timeline Shell

The Timeline tab shows a compact header, an active-agent selector, and a lazy
vertical card list.

If one active agent exists, Timeline selects it automatically and shows the card
list immediately. If multiple active agents exist, Timeline shows a selector with:

- agent title
- agent ID
- provider
- current stage label
- task or session summary
- status
- latest event age

The default selection is the first active agent in deterministic control-plane
selection order. The order is owned by daemon readback, not by Swift sorting
against partial local data:

```text
selection_order ASC
agent_id ASC
```

`selection_order` is computed by the control plane from frozen workflow stage
order, then agent start time, then agent ID. If the backend cannot compute stage
order from the frozen workflow snapshot, it must return `selection_order = null`
and mark the selector result unavailable instead of letting Swift infer a
different order.

### Card Ordering

Cards are sorted newest first. New live response content updates the existing
response card in place rather than creating a new card per chunk.

For a selected agent, the order is:

1. newest active or completed event
2. previous events from the same selected agent
3. oldest retained event

The order is visual only. The underlying persisted event timestamps and IDs
remain unchanged.

### Collapsed Cards

Collapsed cards have a standard size and stable layout. They show:

- title, such as `Agent response`, `Agent response complete`, `Prompt sent`, or
  `Tool call`
- event kind badge
- agent ID
- provider ID
- copyable event ID
- compact status summary
- optional count metadata, such as character count, chunk count, command count,
  or tool result status

Collapsed completed `Agent Response` cards must not show the response tail. They
show a summary such as:

```text
Response complete - 18 chunks - 42.1 KB
```

Collapsed streaming `Agent Response` cards may show progress metadata, but should
not replace the full response with a moving tail that makes earlier chunks appear
lost. The accumulated response is inspectable through the expanded bounded
preview, and the full raw accumulated response remains copyable/readable through
the control-plane detail path.

State and time are hidden in the normal card body. They appear in a hover/focus
metadata strip and are exposed to accessibility.

### Expanded Cards

Clicking a card expands it. Clicking it again collapses it. Clicking another card
collapses the currently expanded card and expands the clicked card.

Expanded cards show:

- the formatted render preview for the card detail
- copy controls for event ID and full raw content
- state, timestamp, stage, agent ID, provider, and session generation metadata
- content size and digest when available

Expanded content is bounded in height and scrolls internally when needed, so one
large provider response cannot push the whole run detail view into an unusable
state.

The raw detail remains copyable even when the formatted render preview is capped.
This distinction is intentional: operators can inspect or copy full evidence
without forcing SwiftUI to lay out unbounded text.

### Formatting

Timeline uses a native SwiftUI formatter, not a web view. The formatter should:

- preserve plain text line breaks
- render fenced code blocks in a monospaced block
- detect valid JSON objects or arrays and pretty-print them
- render inline code spans with monospaced styling when practical
- keep invalid markdown or invalid JSON as plain text
- provide copy buttons for code blocks and full raw content
- use Forge colors and typography tokens

Formatting is applied lazily when a card is expanded. Collapsed cards render only
summary text.

### Metadata and Copying

Every card has a stable event ID. The ID is visible in compact form and can be
copied with a single click or keyboard action.

State and timestamp are not visible by default. They appear on hover and keyboard
focus as a metadata strip:

```text
State: state_10_implementation_refined
Time: 2026-05-21 08:44:47
Agent: code_writer
Provider: claude
Session: 226c2b51-eaf5-4257-b3ba-10940216e064
```

The hover-only behavior must have an accessible keyboard equivalent.

### Multi-Agent Selection

Timeline is scoped to one active agent at a time. When multiple agents are active
in the selected run, the operator can switch active agents through a selector
above the card list.

The selector should be compact, readable, and pleasant to use. It should not
force operators to scan raw IDs first. A good option is a segmented or list-like
control with:

- agent title as primary text
- provider badge
- current stage/task as secondary text
- live event count or latest activity

Switching agents changes the card list without losing the expanded-card behavior
for the newly selected agent. If the previously expanded event is not part of the
new agent selection, it collapses.

## Readback Contract

The Swift app should continue to consume control-plane readback. The presentation
layer may introduce a dedicated `P093TimelineCardPresentation` model, but it must
be derived from daemon data.

Each timeline card needs:

```text
id
run_id
agent_id
agent_title
provider_id
stage_id
stage_label
session_generation_id
event_kind
event_title
summary
raw_detail
detail_digest
detail_char_count
chunk_count
is_streaming
is_terminal
occurred_at
state_label
copyable_identifier
```

The active-agent selector needs:

```text
agent_id
agent_title
provider_id
stage_id
stage_label
task_label
status
session_generation_id
started_at
last_event_at
event_count
selection_order
selection_unavailable_reason
```

If existing P031/P036 readback already provides these fields, the Swift presenter
can derive the card model locally. If any field is missing, the control-plane
readback should be extended rather than inferring from Swift-local state.

### Field Ownership

Every field in the presentation model has one explicit owner. Swift may perform
deterministic formatting and display derivations, but it must not recreate
timeline truth from stage history, local orchestration state, or artifact scans.

| Field | Owner | Source / derivation | Missing behavior | Test owner |
|---|---|---|---|---|
| `id` | daemon | Runtime timeline event ID, or stable coalesced response ID emitted by control-plane readback | drop card as invalid | GraphQL/read-model |
| `run_id` | daemon | runtime event readback | drop card as invalid | GraphQL/read-model |
| `agent_id` | daemon | active agent execution / runtime event attribution | drop card as invalid | GraphQL/read-model |
| `agent_title` | daemon with Swift display fallback | catalog snapshot title for `agent_id`; Swift may display `agent_id` if title is null | show `agent_id` as title fallback | GraphQL/read-model + Swift presenter |
| `provider_id` | daemon | runtime event / agent execution provider | drop card as invalid | GraphQL/read-model |
| `stage_id` | daemon | runtime event stage attribution | show metadata unavailable, do not infer | GraphQL/read-model |
| `stage_label` | daemon with Swift display fallback | frozen workflow snapshot label for `stage_id`; Swift may display `stage_id` if label is null | display `stage_id`; no local lookup | GraphQL/read-model + Swift presenter |
| `session_generation_id` | daemon | runtime session generation attribution | show unknown session | GraphQL/read-model |
| `event_kind` | daemon | runtime event kind vocabulary | show unsupported event card | GraphQL/read-model |
| `event_title` | Swift deterministic derivation | map `event_kind` plus terminal/streaming flags to copy | fallback to `event_kind` | Swift presenter |
| `summary` | Swift deterministic derivation | from daemon `event_kind`, terminal/streaming flags, `detail_char_count`, `chunk_count`, provider, and status | generic summary, no raw tail | Swift presenter |
| `raw_detail` | daemon | runtime event detail or coalesced response content | expansion shows unavailable detail | GraphQL/read-model |
| `detail_digest` | daemon preferred, Swift allowed | daemon SHA-256 over `raw_detail`; Swift may compute only for formatter cache when daemon omits it | cache key falls back to event ID + count | GraphQL/read-model + Swift formatter |
| `detail_char_count` | daemon preferred, Swift allowed | daemon byte/character count; Swift may count `raw_detail` for display | omit count | GraphQL/read-model + Swift presenter |
| `chunk_count` | daemon preferred, Swift allowed for coalesced in-memory response | response chunk accumulator count | omit count | GraphQL/read-model + Swift buffer |
| `is_streaming` | daemon | terminal status from runtime event/readback | fail closed to non-streaming only if terminal is explicit | GraphQL/read-model |
| `is_terminal` | daemon | terminal status from runtime event/readback | fail closed: do not label complete | GraphQL/read-model |
| `occurred_at` | daemon | runtime event timestamp | card remains but no visible time until metadata | GraphQL/read-model |
| `state_label` | daemon | runtime event stage/state label or state ID | hover metadata says unavailable | GraphQL/read-model |
| `copyable_identifier` | Swift deterministic derivation | compact display of `id`; copy action uses full `id` | copy action unavailable if `id` invalid | Swift presenter |

Selector fields:

| Field | Owner | Source / derivation | Missing behavior | Test owner |
|---|---|---|---|---|
| `agent_id` | daemon | active agent execution | omit selector entry | GraphQL/read-model |
| `agent_title` | daemon with Swift display fallback | catalog snapshot title | display `agent_id` | GraphQL/read-model + Swift presenter |
| `provider_id` | daemon | active agent execution provider | omit selector entry | GraphQL/read-model |
| `stage_id` | daemon | active agent execution / current stage attribution | show unknown stage | GraphQL/read-model |
| `stage_label` | daemon with Swift display fallback | frozen workflow snapshot label | display `stage_id` | GraphQL/read-model + Swift presenter |
| `task_label` | daemon | active agent execution task label when available | omit secondary task text | GraphQL/read-model |
| `status` | daemon | active agent execution status | omit selector entry if not active | GraphQL/read-model |
| `session_generation_id` | daemon | active runtime session generation | show unknown session | GraphQL/read-model |
| `started_at` | daemon | active agent execution start time | `selection_order` must still be supplied, otherwise unavailable | GraphQL/read-model |
| `last_event_at` | daemon | max timeline event timestamp for that agent | show no latest-activity text | GraphQL/read-model |
| `event_count` | daemon | count of retained timeline events for that agent | show no count | GraphQL/read-model |
| `selection_order` | daemon | frozen stage order, then start time, then agent ID | mark selector unavailable | GraphQL/read-model |
| `selection_unavailable_reason` | daemon | reason code when `selection_order` cannot be computed | null when available | GraphQL/read-model |

Compatibility rule: new daemon-owned fields must be additive GraphQL/read-model
fields. Older daemons that do not expose them put Timeline into a degraded
readback state with clear copy, not a Swift-local fallback.

Ordering rule: selector ordering is backend-owned. Swift must render agents in
the order returned by readback, using `selection_order` only for diagnostics and
tests. Backend tests must include two active agents where stage order differs
from `started_at` order, and equal-order ties must fall through to `agent_id`.

## Performance Requirements

Timeline must stay responsive under large output.

Required behaviors:

- Use stable card identity while streaming chunks.
- Coalesce response chunks into one response card.
- Keep a bounded retained event list per active agent.
- Render collapsed cards from summaries only.
- Parse markdown and JSON only for the expanded card.
- Cache formatted output by event ID and content digest.
- Use lazy stacks or equivalent virtualized rendering.
- Keep at most one expanded rich renderer alive.
- Avoid layout changes that repeatedly resize the entire run detail surface.
- Respect Reduce Motion by avoiding spatial animation for expansion changes.

### Concrete Budgets

P093 keeps Timeline readable by separating raw evidence retention from render
preview work.

| Budget | Value | Behavior when exceeded |
|---|---:|---|
| retained raw detail per coalesced response | 512 KiB | keep daemon/raw copy path or digest readback; Swift render preview is capped |
| expanded formatted preview input | 96 KiB | render first 96 KiB and show `Preview truncated - copy full raw content` |
| collapsed summary input | 0 raw detail bytes for terminal responses | summary only; no response tail |
| collapsed streaming body | metadata only | no moving tail; expansion shows accumulated detail |
| code block render preview | 32 KiB per block | render capped block with copy-full control |
| JSON pretty-print input | 64 KiB | over limit falls back to plain monospaced text |
| formatter parse time target | 50 ms per expansion on a normal dev laptop | timeout/fallback to plain text |
| formatted cache entries | 32 expanded-card render results | least-recently-used eviction |
| expanded card max height | 560 pt | internal scrolling only |
| retained visible events per selected agent | 40 cards | older events omitted from the focused list |

Formatter cache key:

```text
event_id + detail_digest + formatter_version
```

If `detail_digest` is missing, the cache key falls back to:

```text
event_id + detail_char_count + chunk_count + formatter_version
```

Invalid markdown, invalid JSON, or over-budget content must fall back to plain
text. Fallback is not an error state; it is the expected safe behavior for dense
provider output.

## Accessibility Requirements

- Expansion works by mouse, keyboard, and VoiceOver action.
- Hover metadata also appears on keyboard focus.
- Copyable IDs have accessible labels.
- Status and provider badges have text labels, not color-only meaning.
- Code blocks and JSON blocks preserve readable contrast in light and dark mode.

## Acceptance Criteria

The proposal is implemented when all of the following are true:

1. `Agent Response` chunks accumulate into one stable live card.
2. Completed response cards show only summary metadata while collapsed.
3. Expanded response cards show the bounded formatted preview and expose full
   raw accumulated content through copy/readback.
4. `Prompt sent` and other event cards use the same expand/collapse model.
5. Only one card can be expanded at a time.
6. The card list is newest-first.
7. State and time are hidden until hover or keyboard focus.
8. Event ID is visible in compact form and copyable.
9. Agent ID and provider are visible on every card.
10. Multiple active agents can be selected through a dedicated selector.
11. Single-active-agent runs show the timeline immediately without extra choice.
12. Fenced code blocks and JSON are formatted in expanded cards.
13. Large responses obey the concrete render budgets and over-limit fallback
    behavior.
14. Multi-agent default selection uses backend-owned `selection_order`, including
    stage-order-over-start-time and agent-ID tie-break tests.
15. Timeline data comes from control-plane readback, not Swift-local
    orchestrator state.
16. Missing daemon-owned fields fail closed into degraded readback, not
    Swift-local timeline or stage inference.

## Test Plan

### Swift Unit and Source Tests

Add focused tests for:

- response chunks append into one stable card identity
- completed response collapsed summary does not contain response tail
- expanded response exposes bounded formatted preview plus full raw copy/readback
- prompt cards expand and collapse
- clicking a second card collapses the first card
- newest-first ordering
- state/time are metadata-only in the collapsed body
- event ID copy action exists
- keyboard expansion/collapse action exists for every card type
- VoiceOver action exposure exists for expand/collapse
- copyable event IDs have accessible labels
- status/provider badges expose non-color accessible labels
- agent ID and provider are present in every card model
- multiple active agents default to the first deterministic agent
- selector order is rendered from backend order, not local re-sorting
- switching active agents filters the card list
- JSON and fenced-code formatting
- large response formatting is lazy and scoped to the expanded card
- over-budget markdown/JSON falls back to capped plain text with copy-full control
- formatter cache key uses event ID plus digest, with the documented fallback key

### Control-Plane Readback Tests

Add or update tests proving:

- active timeline events include agent ID and provider
- multiple active agents are reported distinctly
- selector entries include `selection_order` or an explicit unavailable reason
- selector ordering prefers frozen workflow stage order over start time
- equal stage/start tie-breaks by agent ID
- every daemon-owned card and selector field in the ownership table has a
  read-model fixture
- missing daemon-owned fields produce degraded/unavailable readback, not local
  inference
- event ordering is deterministic
- timeline events are not inferred from stage history or Swift-local state

### Proposal Gate

Add `./scripts/test-gate.sh proposal-093` with:

- Swift presentation tests
- readback contract tests
- source guard against Swift-local orchestrator timeline data
- formatting fixtures for markdown fences and JSON
- performance-oriented large-response fixture with 96 KiB render preview,
  64 KiB JSON pretty-print cutoff, 32 KiB code-block cutoff, and over-limit
  fallback assertions
- multi-agent selector-order fixture where stage order differs from start order
- accessibility inspection covering keyboard expand/collapse, VoiceOver actions,
  copyable-ID labels, and non-color badge semantics

Document the gate in `docs/reference/test-gates.md`.

### Remote UI Proof

Remote UI tests remain remote-only by repository policy. The UI proof should
cover:

- single active agent auto-selection
- multi-agent selector
- newest-first order
- expand/collapse toggle
- keyboard expand/collapse toggle
- one-expanded-card invariant
- formatted JSON or fenced code in an expanded card
- completed response summary in collapsed mode
- hover or focus metadata
- copyable-ID accessible label
- status/provider badges readable without color

## Rollout Plan

1. Add the presentation model and unit tests.
2. Add the formatter behind the Timeline card expansion path.
3. Add active-agent selector presentation.
4. Add the proposal gate and reference documentation.
5. Run local proposal gate.
6. Run remote UI proof.
7. Update `docs/reference/macos-operator-navigation.md` after implementation is
   verified.

## Risks

- Large provider responses can cause slow layout if formatted eagerly.
- Markdown and JSON formatting can hide raw evidence if invalid input is
  over-normalized.
- Hover-only metadata can be inaccessible without a focus equivalent.
- Multi-agent selection can become noisy if labels prefer IDs over human-readable
  task context.
- A too-rich Timeline can drift into a stage-history duplicate; this must remain
  selected-active-agent activity only.

## Design Decision

Collapsed Timeline cards prioritize operational summary over raw transcript
content. Expanded cards render a bounded formatted preview, while full raw text
is available through copy/readback. This avoids the current failure mode where a
moving tail makes accumulated provider output look like it has been lost, without
requiring SwiftUI to render unbounded provider text.
