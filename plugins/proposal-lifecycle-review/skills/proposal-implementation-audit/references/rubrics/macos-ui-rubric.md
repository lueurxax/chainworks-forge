# Implementation Audit Addendum

Use this rubric inside `$proposal-implementation-audit`. Compare the proposal obligations, prior proposal-review findings, and actual implementation evidence. Do not fail a `REQ-*` item unless the proposal explicitly committed to that behavior; record other risks as routed specialist findings. Prefer direct code, diff, test-run, runtime, schema, migration, telemetry, benchmark, or log evidence over inference.

---

# UI Review Rubric: macOS Perspective

Evaluate proposal or code-mapped visual design quality for macOS flows. Work from proposal states, repo-local docs, the reusable baseline when present, and current repo surfaces first. Runtime evidence is valuable in implementation audit when live behavior is the claim; distinguish runtime proof from code-only inference.

## Focus Areas

1. Information density and window ergonomics
- Does the layout use desktop space well without turning into a wall of controls?
- Do resizable windows, split views, and sidebars remain coherent?

2. Desktop-native interaction model
- Are menus, toolbars, context menus, keyboard shortcuts, and focus behavior aligned with macOS expectations?
- Does the proposal avoid phone-style patterns that feel cramped on desktop?

3. State presentation
- Are loading, empty, error, disabled, and recovery states still clear in multi-pane layouts?

4. Platform fidelity
- Does the proposal respect multiwindow, selection, drag-and-drop, and pointer/keyboard workflows where relevant?

5. Accessibility
- Are keyboard navigation, focus order, VoiceOver, contrast, and non-color cues considered?

## Output Requirements

For each finding, include:
- Finding ID
- Severity
- Evidence IDs
- Why it matters
- Recommended fix
- Acceptance criteria
- Confidence
