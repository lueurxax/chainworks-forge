# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# API Contract Review Rubric

Evaluate compatibility and contract design quality for request/response, protobuf, event, or schema changes.

## Focus Areas

1. Versioning and backward compatibility
2. Error model consistency
3. Pagination, idempotency, and consumer expectations
4. Migration and rollout safety
5. Cross-stack consumer impact: Apple clients, Rust services, Go services, other downstream systems

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
