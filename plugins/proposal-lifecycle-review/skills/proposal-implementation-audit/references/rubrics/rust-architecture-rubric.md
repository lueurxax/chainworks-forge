# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Architecture Review Rubric: Rust Service Perspective

Evaluate technical architecture quality and implementation risk for a Rust service or backend proposal.

## Focus Areas

1. Workspace and crate boundaries
- Is ownership of crates, modules, and public APIs clear?
- Are new crates justified or just moving confusion around?

2. Trait and API design
- Are interfaces testable and explicit about error and ownership semantics?
- Is the proposal keeping transport, domain, and persistence seams separate?

3. Async/runtime seams
- Executor boundaries, task ownership, cancellation, startup/shutdown, blocking work isolation

4. Persistence and contract design
- DB, queue, cache, file, schema, and protocol ownership
- Backward compatibility and migration shape

5. Operability and testability
- Tracing, metrics, failure injection, reproducible tests, clear deployment seams

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
