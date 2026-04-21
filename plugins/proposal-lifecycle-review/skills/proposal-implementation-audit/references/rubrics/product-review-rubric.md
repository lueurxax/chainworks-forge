# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# Product Review Rubric: Staff PM Perspective

Evaluate whether the proposal is solving the right problem for the right user with measurable outcomes and a credible rollout path, based on repo-local materials, the reusable baseline when present, code inspection, current repo reality, and any provided data.

Product review is evidence-driven and optional.

If necessary product evidence is missing, report evidence gaps rather than speculate. In proposal-first review, lack of runtime evidence does not block product review by itself.

## Focus Areas

1. Problem framing and target segment
2. User value vs current state
3. Outcome logic and metrics
4. Scope sharpness and sequencing
5. Instrumentation and experiment design
6. Business, trust, and operational risk

## Output Requirements

For each finding, include:

- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Leading metric
- Guardrail metric
- Decision checkpoint
- Confidence
