# Proposal 021: Run Transition Notifications and Attention Routing

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Goose |
| Depends on | [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/operator-experience.md](../reference/operator-experience.md), [../reference/runtime-contract.md](../reference/runtime-contract.md) |
| Scope | Add canonical run/stage notification events, global notification preferences, in-app transition feed, and attention-only macOS notifications. |
| Goal | Make run progress visible without forcing operators to stare at the shell constantly, while avoiding a second truth system or noisy notification spam. |

---

## 1. Context and Motivation

The current shell makes operators poll run progress manually. This causes two problems:

1. Important events like approvals, blocks, failures, and completion can be missed or noticed late.
2. Low-signal progress changes are visible only if the operator happens to be watching the UI.

The system already has persisted run and stage truth, approval flow, blocked-run recovery, and report surfaces. What is missing is a canonical notification layer that routes those truth changes into:

- a lightweight in-app feed for all transitions
- system-level macOS notifications only for events that require or justify human attention

This proposal does **not** introduce a new authority for state. It only adds a delivery layer on top of existing canonical run/stage truth.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can an operator enable notifications globally without per-run setup?
2. Can the shell show all run transitions in-app without spamming macOS notifications?
3. Can attention-requiring events reliably surface as system notifications?
4. Can notification delivery survive relaunch without replaying stale historical events as new ones?
5. Can notification routing stay subordinate to canonical run/stage truth rather than UI heuristics or report regeneration?

---

## 3. Scope

This proposal includes:

- global notification preferences
- a canonical `RunNotificationEvent` domain type
- a shell-owned in-app notification feed
- macOS notifications for attention-required events
- dedupe and replay protection
- navigation from notification surfaces back into the existing shell

This proposal does **not** include:

- email, Slack, SMS, or push notifications
- per-run subscription controls
- a separate notification product or inbox app
- notification-authored recovery decisions
- speculative notifications derived from ambiguous state

---

## 4. Core Product Behavior

### 4.1 Delivery model

The default behavior is hybrid:

- **In-app notifications** receive **all canonical transitions**
- **System notifications** receive **attention-only events**

The canonical attention set is:

- `approvalRequired`
- `runBlocked`
- `runFailed`
- `runCompleted`

`stageTransitioned` is in-app only by default.

### 4.2 Preference scope

Notification settings are global application settings, not per-run settings.

They apply to all runs visible to the operator shell.

The initial configuration should be:

- in-app notifications: `all_transitions`
- system notifications: `attention_only`
- notifications enabled: `true`

### 4.3 Operator expectation

An operator should be able to:

- keep the app open and see a feed of run changes in-app
- switch away from the app and still be alerted when human attention is needed
- click or open the notification target and land back in the existing run / approval / blocked surface

---

## 5. Architecture

### 5.1 Canonical notification authority

The source of truth remains the persisted run/stage/approval model described by the existing runtime and execution-truth references.

Notifications must never become a second state authority.

The notification layer is a normalized delivery view of already-canonical state changes.

### 5.2 New domain types

#### `RunNotificationPreferences`

Global persisted user settings:

```swift
struct RunNotificationPreferences: Codable, Sendable {
    var enabled: Bool
    var inAppMode: InAppNotificationMode
    var systemMode: SystemNotificationMode
}
```

Modes:

- `InAppNotificationMode.allTransitions`
- `InAppNotificationMode.disabled`
- `SystemNotificationMode.attentionOnly`
- `SystemNotificationMode.disabled`

The initial implementation does not need extra variants beyond these.

#### `RunNotificationEvent`

Canonical normalized event emitted from persisted truth:

```swift
struct RunNotificationEvent: Codable, Hashable, Sendable {
    let eventID: String
    let runID: UUID
    let kind: RunNotificationEventKind
    let stageID: String?
    let stageLabel: String?
    let summary: String
    let detail: String?
    let createdAt: Date
    let attentionRequired: Bool
}
```

Kinds:

- `stageTransitioned`
- `approvalRequired`
- `runBlocked`
- `runFailed`
- `runCompleted`

### 5.3 New coordinator

`RunNotificationCoordinator` is responsible for:

- observing canonical run/stage truth changes
- normalizing those changes into `RunNotificationEvent`
- deduplicating replayed or stale events
- routing events into the in-app feed and macOS notification adapter

It does **not** own run-state derivation, recovery choice, or status inference.

### 5.4 Delivery channels

#### `InAppNotificationFeed`

Shell-owned feed for all canonical transitions.

Expected qualities:

- lightweight
- navigable
- recent-history oriented
- attached to the current app shell rather than a new standalone product surface

#### `SystemNotificationAdapter`

Thin adapter over the macOS notification APIs.

It should only receive events that already passed canonical event normalization and attention routing.

---

## 6. Event Semantics

### 6.1 `stageTransitioned`

Emit only when the canonical current stage changes to a different stage identity.

Do not emit for:

- UI refresh
- report rebuild
- projection recalculation
- relaunch restore without a real stage change

This event is in-app only by default.

### 6.2 `approvalRequired`

Emit when the run enters canonical `waitingApproval`.

This event is:

- in-app
- system-notified

### 6.3 `runBlocked`

Emit when the run enters canonical blocked state with a real blocker reason.

This event is:

- in-app
- system-notified

### 6.4 `runFailed`

Emit only for canonical terminal failure when the system distinguishes this from blocked/interrupted states.

This event is:

- in-app
- system-notified

### 6.5 `runCompleted`

Emit only for canonical successful terminal completion.

This event is:

- in-app
- system-notified

### 6.6 Fail-closed rule

If state is ambiguous, no notification event may be emitted.

Examples:

- `ready` with `currentStageID == nil`
- stale `running` after relaunch without a current execution owner
- contradictory report/regeneration state that does not match canonical run truth

The system must prefer silence over false notification certainty.

---

## 7. Dedupe and Replay Rules

Notifications must not replay old truth as if it were new.

The coordinator must persist and compare a dedupe identity derived from canonical state, for example:

- `runID`
- event kind
- stage identity or stage lineage
- terminal sequence / event version

This prevents duplicates from:

- relaunch
- report regeneration
- projection refresh
- repeated shell rendering

Historical events may still exist in the in-app feed, but they must not be re-delivered as fresh macOS notifications.

---

## 8. Settings and Permissions

### 8.1 Global settings location

Notification preferences live in `Settings`.

The operator should be able to:

- enable or disable notifications globally
- enable or disable in-app transition feed
- enable or disable system attention notifications

### 8.2 System permission behavior

When system notifications are enabled for the first time, the app requests macOS notification permission.

If permission is denied:

- the in-app feed continues to function
- the system channel degrades to disabled
- the app should surface the disabled state clearly in settings

Permission failure must not block core run execution.

---

## 9. Shell Ownership

Notification UI belongs to the existing operator shell.

The first implementation should extend current shell-owned surfaces rather than introduce a parallel notification product.

Examples of acceptable ownership:

- app-shell badge or indicator
- lightweight recent event list
- deep-link navigation into existing run, approval, blocked, and report surfaces

Examples explicitly out of scope:

- a separate notification workspace
- a second recovery console
- notification-authored run actions outside the current shell ownership model

---

## 10. Rollout Plan

1. **Phase 1: Canonical event normalization**
   - Implement `RunNotificationEvent`
   - Implement dedupe identity and fail-closed rules
   - Emit in-app events only

2. **Phase 2: Global settings and shell feed**
   - Add global settings
   - Add shell-owned in-app feed and navigation

3. **Phase 3: macOS system notifications**
   - Add permission handling
   - Deliver attention-only events through the system channel

4. **Phase 4: Proof and hardening**
   - Verify relaunch dedupe
   - Verify blocked/approval/completed attention routing
   - Verify no false notifications from ambiguous state

---

## 11. Acceptance Criteria

The proposal is complete when:

1. The app has global notification preferences in `Settings`.
2. The in-app notification feed can show all canonical run transitions.
3. macOS notifications are emitted only for the canonical attention-required set:
   - `approvalRequired`
   - `runBlocked`
   - `runFailed`
   - `runCompleted`
4. Notifications are derived from canonical persisted run/stage truth, not UI heuristics.
5. Relaunch, report regeneration, and projection refresh do not re-emit stale notifications as new ones.
6. Notification surfaces remain shell-owned and route back into the current run/approval/block/report surfaces.
7. Ambiguous state produces no notification event.

---

## 12. Alternatives Considered

### Per-run subscription model

Rejected for the first slice.

It adds operator complexity and weakens the guarantee that important attention-required events are globally visible.

### System notifications for all transitions

Rejected because it would create noisy, low-value spam for normal stage motion.

### Notification logic embedded directly in views

Rejected because it would couple delivery to UI refresh timing and create a second, heuristic truth lane.

