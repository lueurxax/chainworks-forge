# Swift macOS Boundary Contract (P081)

**Status**: P081 Swift/operator-shell support is implemented for the boundary contract. The Rust matrix/fixture/validator, `audit_log` and `audit_log_checkpoints` storage, `CallerClass` enum, `CallerContext.caller_class`, and principal-table `schema_version 3` reader have landed. The daemon-injected `BoundaryPolicy` is wired into GraphQL/MCP with mode-aware semantics. `approval_mutation_idempotency` plus request hashing backs approval mutation replay, and `RunsHomeView` injects `P081ApprovalActionAttemptStore`, which owns one UUIDv7 idempotency key per `(approvalID, action)`, persists pending attempts across app restarts through `UserDefaults`, reuses keys across retries, and clears the key after terminal success. Swift readback preserves typed `extensions.redactions` and maps ordinary nil, redacted nil, and `drop_resource` to distinct `P081RedactionState` accessibility metadata. Backend `operatorAlerts` and MCP `operator.alerts.list` expose bounded safe-mode/tamper alerts with lifecycle/native-delivery metadata; native macOS escalation is driven from that projection. The GraphQL WebSocket contract uses `4401` for unauthorized `connection_init`, `4403` for forbidden caller class or subscription surface policy, and `4408` for init timeout/malformed first-frame and policy reload (`POLICY_RELOAD`). See [boundary-first-api-auth-contract.md](boundary-first-api-auth-contract.md) for the rollout phase table.

This document details the contract for Swift macOS UI interactions with the Boundary Policy, covering accessibility, approval action management, macOS command integration, redaction handling, and window state behavior. It directly reflects the `swift_macos_boundary_contract` and `observability_operator_contract.alert_delivery_macos` sections of Proposal 081.

## Accessibility Contract

The UI must adhere to the following accessibility guidelines for various states:

*   **`actionability_false`**: Approve and Reject controls must remain discoverable with Full Keyboard Access when visible, clearly expose their disabled state, use `accessibilityValue` from `disabledReasonCode`, and never advertise an actionable default button trait.
*   **Contrast and Motion**: `Increase Contrast` settings must use non-color indicators (e.g., lock icons, text labels, border treatments). `Reduce Motion` must disable alert pulse or shake animations, using static badges instead. State should never be communicated solely by color.
*   **`drop_resource`**: When a resource is dropped due to policy, the UI must expose an `accessibilityLabel` "Restricted view", an `accessibilityValue` from `denial_copy`, and an `accessibilityHint` with human-readable troubleshooting text. These elements must remain reachable by VoiceOver and Full Keyboard Access without exposing stale child controls.
*   **Ordinary `nil`**: An ordinary `nil` value for a field should expose the field's display name and an `accessibilityValue` of "No value," without restricted hints or locked traits.
*   **Reason Codes**: Known reason codes for denials should map to predefined `denial_copy` strings. Unknown future codes should render "Action Not Available" while copied diagnostics preserve the raw `reason_code`.
*   **Redacted `nil`**: A redacted `nil` value must expose an `accessibilityLabel` equal to the field display name, an `accessibilityValue` of "Restricted value", and an `accessibilityHint` stating "Permissions hide this value. Copy diagnostics for the access rule." It should also display a locked or disabled trait appropriate to the control, without implying the value is merely empty.
*   **Required Tests**:
    *   `accessibility_redaction_parity`
    *   `full_keyboard_access_redacted_nil_vs_ordinary_nil`
    *   `keyboard_full_access_actionability_false`
    *   `increase_contrast_redaction_state`
    *   `reduce_motion_alert_state`

## Approval Action Attempt Store

The `ApprovalActionAttemptStore` is responsible for managing idempotency keys for approval actions. The current implementation is `P081ApprovalActionAttemptStore`, injected into `P031ThinReadDashboardModel.bootstrap(...)`. It owns one UUIDv7 `idempotencyKey` per `approval_id/action` attempt, reuses it across retries and duplicate-tap suppression, persists pending attempts across app restarts or network loss through `UserDefaults`, and clears after terminal success. Reload-confirmed conflict clearing remains part of the later `4408 POLICY_RELOAD` work.

## macOS Commands

The following macOS commands and their behaviors are specified:

*   **Default Shortcuts**:
    *   `Approve Approval`: Command-Return (when focused approval is actionable)
    *   `Reject Approval`: Command-Shift-Return (when focused approval is actionable)
    *   `Copy Boundary Diagnostics`: Command-Option-C
*   **Menu Titles**:
    *   "Approve Approval"
    *   "Reject Approval"
    *   "Copy Boundary Diagnostics"
*   **Validation Source**: Menus and the command palette must validate against `ActionabilityProjection.availableActions` and the front-most key window selection. If multiple windows are open, the front-most focused approval row owns the target; otherwise, commands are disabled.
*   **Required Tests**:
    *   `keyboard_driven_approval_uses_actionability_projection`
    *   `copy_boundary_diagnostics_excludes_secrets`

## Redaction Envelope

Typed GraphQL envelope decoding preserves `extensions.redactions` before SwiftUI rendering through `P031GraphQLResponseDecoder.decodeExtensions(from:)`. A redacted nullable `nil` transforms into `P081RedactionState.redacted` with `redactionId`, `reasonCode`, `rowId`, `path`, and `callerClass`. An ordinary `nil` remains `P081RedactionState.ordinaryNil`. `drop_resource` is represented as `P081RedactionState.dropResource`, which exposes "Restricted view" / "Permission denied" accessibility metadata and prevents stale selected-detail content from being treated as readable.

## Window State

`boundaryRuntime.safeModeActive` drives the backend alert projection and should drive a window-level toolbar badge or persistent banner in every `Scene`. State restoration after a `4408 POLICY_RELOAD` event preserves the selected run ID, selected approval ID, scroll anchor where available, and open diagnostics tab through `SceneStorage` or `NSUserActivity`; the control-plane contract for that reload is close code `4408` and reason `POLICY_RELOAD`.

## macOS Native Alert Delivery

Critical alerts must remain visible even when the main window is hidden or inactive, driven by the `operatorAlerts` readback contract:

*   **Authorization Timing**: The app requests `UNUserNotificationCenter` authorization during operator setup before Phase 4 enforcement or when enabling local operator alerts, not immediately when a critical alert fires. The app records authorization state in operator alert settings and surfaces degraded native delivery if notifications are denied.
*   **Dock and Status Item**: Critical and error alerts update the Dock badge and status item from the same `operatorAlerts` projection. Clearing the alert removes its contribution to the badge. If the app normally hides the status item, a temporary status item is shown while a critical alert is active.
*   **Severity to Surface Mapping**:
    *   **Critical**: `operatorAlerts` inbox, non-dismissible persistent in-app banner, Dock badge count, `MenuBarExtra` or `NSStatusItem` critical state (even when the main window is closed), `NSApp.requestUserAttention(.criticalRequest)` when inactive, `UNUserNotification` with critical interruption (if authorized).
    *   **Error**: `operatorAlerts` inbox, persistent in-app banner, Dock badge count, `MenuBarExtra` or `NSStatusItem` warning state, `NSApp.requestUserAttention(.informationalRequest)` when inactive, `UNUserNotification` with timeSensitive interruption (when authorized).
    *   **Info**: `operatorAlerts` inbox, nonmodal in-app banner (while app is foreground).
    *   **Warn**: `operatorAlerts` inbox, persistent in-app banner, toolbar or window chrome badge on every open operator window.
*   **Silence Semantics**: Silencing uses the alert's `dedupe_key` and expiry. Silence suppresses new UN notifications and `requestUserAttention` for that `dedupe_key` until `silence_until_ms`, but the inbox entry, safe-mode banner, and diagnostics remain visible. Critical safe-mode alerts cannot be permanently dismissed while the condition remains active.
*   **Required Tests**:
    *   `operator_alert_fires_and_clears_hidden_window`
    *   `operator_alert_silence_suppresses_native_escalation_until_expiry`
    *   `operator_alert_dock_status_item_clear_on_recovery`
