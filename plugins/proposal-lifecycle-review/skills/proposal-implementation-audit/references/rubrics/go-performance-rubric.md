# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Performance Review Rubric: Go Service Perspective

Evaluate hot-path risk and measurement quality for a Go service or microservice proposal.

## Focus Areas

1. Allocation churn and GC pressure
2. Serialization and network overhead
3. Lock contention, batching, and pooling strategy
4. Cache behavior and downstream load shape
5. Measurement plan: benchmark, trace, or production signal suitable for the actual risk

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
