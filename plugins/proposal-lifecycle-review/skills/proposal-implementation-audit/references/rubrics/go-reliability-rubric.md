# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Reliability Review Rubric: Go Service Perspective

Evaluate failure handling and operational resilience for a Go service or microservice proposal.

## Focus Areas

1. Deadline, timeout, and cancellation handling
2. Goroutine lifecycle and leak risk
3. Retry, dedupe, and idempotency shape
4. Backpressure, queueing, and overload behavior
5. Recovery, observability, and graceful shutdown

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
