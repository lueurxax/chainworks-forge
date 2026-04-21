# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# UI Review Rubric: iOS Perspective

Evaluate proposal or code-mapped visual design quality for iPhone/iPad flows. Work from proposal states, repo-local docs, the reusable baseline when present, and current repo surfaces first. Runtime evidence is valuable in implementation audit when live behavior is the claim; distinguish runtime proof from code-only inference.

## Focus Areas

1. Visual hierarchy and scanability
- Is the primary action obvious?
- Are dense screens still readable without visual shouting?

2. State presentation
- Are loading, empty, error, disabled, and success states specified?
- Do transitions make the current state legible?

3. Navigation and spatial orientation
- Does the proposal fit iOS navigation patterns?
- Are deep-link, back-stack, modal, and tab behaviors coherent?

4. Platform fidelity
- Does the flow feel native to iOS or does it fight the platform?
- Are touch targets, typography, dynamic type, and safe-area behavior respected?

5. Accessibility
- Are VoiceOver, dynamic type, contrast, and non-color status cues considered?

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
