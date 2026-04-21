# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Performance Review Rubric: Rust Service Perspective

Evaluate hot-path risk and measurement quality for a Rust service or backend proposal.

## Focus Areas

1. Allocation and copy behavior
2. Locking, contention, and scheduling overhead
3. Serialization, batching, and streaming cost
4. Cache strategy and data locality
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
