# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# UX Review Rubric: Apple Client Perspective

Evaluate proposal or code-mapped user experience quality for Apple client flows. Work from intended journeys, state coverage, repo-local docs, the reusable baseline when present, and current repo reality. Do not require simulator proof in proposal mode.

## Focus Areas

1. User goals and task flow
- Can the user complete the main task without friction or guesswork?
- Are repeated tasks efficient, not just first-run tasks?

2. Trust and clarity
- Can users explain what is happening, especially when money, permissions, sync status, or destructive actions are involved?

3. Error prevention and recovery
- Are costly mistakes prevented?
- Can the user recover cleanly from the likely errors?

4. Accessibility and inclusivity
- Is the proposal viable with dynamic type, VoiceOver, reduced motion, and keyboard access where applicable?

5. Cognitive load
- Does the flow reduce decision fatigue instead of moving it around?

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
