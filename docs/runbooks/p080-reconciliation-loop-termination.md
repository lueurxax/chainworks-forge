# P080 Reconciliation Loop Termination

**Alert trigger:** `P080ReconciliationLoopTermination`

This metric is informational in the approved P080 vocabulary. Treat it as actionable only when it repeats, pairs with classifier/repair failures, or reports a non-operator-shutdown reason during rollout soak.

## Current Phase 1 Behavior

The live loop runs every 30 seconds with a 20 second tick deadline. `live_disable` does not terminate the task; it causes each tick to skip work fail-closed. With `detection_only` disabled, the loop also skips classification and writes no P080 readback.

## Initial Investigation

1. Group the alert by `reason`: `iteration_deadline`, `gateway_saturated`, `classifier_error`, or `operator_shutdown`.
2. For `iteration_deadline`, check whether DB reads or write-gateway acquisition exceeded the P080 timeout hierarchy.
3. For `gateway_saturated`, inspect control-plane write-gateway health and storage write pressure before attempting any operator action.
4. For `classifier_error`, inspect the paired `stale_execution_classifier_error_total{class,reason}` labels and follow `docs/runbooks/p080-stale-execution-repair.md`.
5. For `operator_shutdown`, verify it matches an intentional daemon shutdown or restart.
6. Confirm `p080_rollout_control_v1.live_disable` and `detection_only` state before interpreting missing readback rows.
