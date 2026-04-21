# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Architecture Review Rubric: Apple Client Perspective

Evaluate technical architecture quality and implementation risk for iOS or macOS flows. Work from proposal text, adjacent docs, the reusable baseline when present, code-path mapping, and current repo reality.

## Focus Areas

1. System boundaries and modularity
- Separation of concerns across presentation, domain, networking, persistence, and app services
- Clarity of ownership between targets, modules, and shared packages

2. State management and data flow
- Source-of-truth strategy
- Sync and cache consistency
- Navigation and deep-link ownership
- State restoration implications

3. Concurrency and lifecycle
- Main-thread safety, actor isolation, task cancellation, background work, app lifecycle boundaries

4. Reliability and testability
- Test seams, deterministic logic boundaries, failure injection points, non-happy-path handling

5. Security and privacy
- Secret handling, local storage, PII boundaries, logging hygiene, permission-state handling

6. Rollout and operability
- Feature flags, telemetry, migration paths, rollback safety, diagnostics

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
- Evidence gaps when the proposal or code mapping is too weak to support a firm call
