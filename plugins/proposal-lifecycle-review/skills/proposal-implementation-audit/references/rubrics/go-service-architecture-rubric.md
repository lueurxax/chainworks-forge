# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Architecture Review Rubric: Go Service Perspective

Evaluate technical architecture quality and implementation risk for a Go service or microservice proposal.

## Focus Areas

1. Package boundaries and ownership
- Clear separation between transport, domain, persistence, and wiring

2. Interface design and dependency flow
- Interfaces exist where they buy testability or substitution, not as ceremony

3. Context propagation and lifecycle
- Request, worker, and shutdown lifecycles are explicit
- Deadlines and cancellation do not get dropped on the floor

4. Persistence and contract seams
- Schema, repository, queue, event, and API ownership are explicit
- Versioning and migrations are credible

5. Operability and testability
- Logs, metrics, traces, health, startup, and failure seams are reviewable

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
