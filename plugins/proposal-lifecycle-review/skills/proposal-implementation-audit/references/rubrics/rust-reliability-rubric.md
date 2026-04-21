# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Reliability Review Rubric: Rust Service Perspective

Evaluate failure handling and operational resilience for a Rust service or backend proposal.

## Focus Areas

1. Failure taxonomy
- Validation, dependency, timeout, shutdown, retryable, and terminal errors are clearly distinguished

2. Idempotency and replay
- Retries, dedupe keys, at-least-once delivery, and replay safety are specified when relevant

3. Backpressure and overload
- Queue depth, admission control, bounded work, and overload behavior are explicit

4. Cancellation and graceful shutdown
- Tasks and workers stop without silent loss, duplication, or deadlock

5. Recovery and diagnostics
- Errors are actionable, observable, and testable

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
