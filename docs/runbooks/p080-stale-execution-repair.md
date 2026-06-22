# P080 Stale Execution Diagnostics And Repair

P080 is currently phase-scoped: detection/readback is enabled behind `detection_only`; the live reconciliation loop and Operator `repair_if_safe` may repair `acp_startup_stale` tuples after that class is enabled in Phase 2+ and `scheduler_ownership_drift` tuples after that class is enabled in Phase 3+. Helper reap, side-effect-adjacent repair, manual hold, and permanent-hold clear remain disabled.

## First Checks

1. Open `docs/evidence/dashboards/p080-overview.json` and inspect the panel matching the alert metric.
2. Query `p080.diagnostics.get.v1` with a narrow `filter.run_id` and, when available, `stage_id`, `work_item_id`, and `stale_class`.
3. Check `p080_readback_v1.projection_integrity`. Do not act on rows with `stale` or `tamper_detected`.
4. Use `repair_if_safe` only for `stale_class=acp_startup_stale` in Phase 2+ or `stale_class=scheduler_ownership_drift` in Phase 3+, after the matching rollout row is enabled. The live loop may already repair those promoted classes; every mutating operator request must include a fresh `operator_request_dedup_key`; exact replays return the stored response, while changed targets or changed rollout/auth fences return `idempotency_conflict` before mutation.
5. Do not manually retry release, publish, git, upload, or distribution work unless P076 side-effect reconciliation reports `retry_safe`.

## Correlation Fields

Every investigation should capture `run_id`, `stage_id`, `work_item_id`, `stale_class`, `projection_generation`, `projection_updated_at`, and `repair_idempotency_key` when present.

## Readback Matrix

| Value | Next operator step | Safe command | Owner |
|---|---|---|---|
| `hold_reason=cooldown_active` | Wait until `next_retry_or_backoff_time`; do nothing. | none | n/a |
| `hold_reason=permanent_hold_active` | Inspect recent repair events; clear only after root cause is resolved. | no current P080 command; future P098-owned clear path | platform on-call |
| `hold_reason=ambiguous_owner` | Inspect helper lease and parent-chain evidence in `details_json`. | `p080.diagnostics.get.v1` filtered to the tuple | platform on-call |
| `hold_reason=side_effect_drift_unsafe` | Wait for P076 to declare `retry_safe`; do not retry side effects. | none; see P076 runbook | release on-call |
| `hold_reason=dependency_read_failure` | Inspect `dependency_read_failure_reason` and dependent subsystem health. | log query for `dependency_read_failure_reason` | platform on-call |
| `hold_reason=gateway_saturated` | Verify the control-plane write gateway is making progress. | none; auto-recovers | platform on-call |
| `hold_reason=live_disable` | Verify intentional kill switch; re-enable only through future rollout control. | future `p080.rollout_control.set.v1` | platform on-call |
| `hold_reason=warmup_pending` | Wait for warmup window to clear. | none | n/a |
| `hold_reason=rollout_disabled` | Verify rollout phase and class enablement. | future `p080.rollout_control.set.v1` | platform on-call |
| `error=unauthenticated` | Re-authenticate; refresh principal credentials. | none | identity on-call |
| `error=unauthorized_missing_capability` | Grant the missing P080 capability if appropriate. | identity console | identity on-call |
| `error=rollout_disabled`, `class_disabled`, or `live_disabled` | Inspect rollout-control state. | future `p080.rollout_control.set.v1` | platform on-call |
| `error=action_disabled_in_phase` | Verify the action is phase-enabled; defer hold/clear actions in Phase 1. | none | platform on-call |
| `error=invalid_cursor` | Re-query without the cursor. | `p080.diagnostics.get.v1` with `cursor=null` | n/a |
| `error=idempotency_conflict` | The dedup key was reused for a different request or after rollout/auth/live-disable state changed. Re-diagnose the tuple, then choose a fresh `operator_request_dedup_key` only if the repair is still intended. | re-issue request with new key | n/a |
| `error=permanent_hold_active` | Inspect active hold readback; clear only after the future hold-clear contract ships. | no current P080 command; future P098-owned clear path | platform on-call |
| `error=predicate_revalidation_failed` | Re-diagnose; state changed since the predicate hash was captured. | re-issue with refreshed predicate hash or omit it | n/a |
| `error=side_effect_unsafe` | Defer to P076; do not retry. | none; see P076 runbook | release on-call |
| `error=dependency_unavailable` | Inspect the named dependency. | subsystem-specific diagnostics | platform on-call |
| `error=unsupported_version` or `version_mismatch` | Align the client schema version and tool name. | re-issue with current schema version | platform on-call |
| `error=enumeration_budget_exceeded` | Narrow filters or omit exact total count. | smaller `p080.diagnostics.get.v1` query | n/a |
| Parser/resource-limit errors | Reduce request size, remove duplicate keys, and normalize input. | corrected MCP request | n/a |
| `error=internal_error` | Retry after `retry_after`; page if persistent. | re-issue after delay | platform on-call |
| `rollout_disablement=phase_not_reached` | Advance phase only after promotion evidence passes. | future `p080.rollout_control.set.v1` | platform on-call |
| `rollout_disablement=class_disabled` | Enable class only after promotion evidence passes. | future `p080.rollout_control.set.v1` | platform on-call |
| `rollout_disablement=live_disabled` | Verify kill switch; re-enable only if unintentional. | future `p080.rollout_control.set.v1` | platform on-call |
| `projection_integrity=stale` | Wait for projection rebuild; do not act on stale rows. | none | platform on-call |
| `projection_integrity=tamper_detected` | Halt P080 operator actions and inspect `command_journal`. | none; incident process | security on-call |
