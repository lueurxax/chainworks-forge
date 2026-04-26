# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Observability and Rollout Review Rubric

Evaluate whether the proposal can be rolled out, measured, debugged, and rolled back without drama.

## Focus Areas

1. Metrics, traces, logs, and event coverage
2. Feature flags, config gating, and dark-launch strategy
3. Rollout sequencing and hold criteria
4. Rollback path and migration recovery
5. Operator visibility: alerting, dashboards, health, and decision checkpoints

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
