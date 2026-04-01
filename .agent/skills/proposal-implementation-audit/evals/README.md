# Lightweight Evals

Use these evals to sanity-check the skill after edits. Keep them lightweight: fresh thread, explicit invocation, minimal repo context, one proposal path, and verification of the generated report.

## What to Check

- Trigger behavior: the skill is used only when explicitly invoked.
- Dual-track workflow: the report contains both the objective `REQ-*` audit and the expert multi-lens review.
- Platform handling: the report records platform scope and audits iOS/macOS conventions separately when both are in scope.
- Contract extraction: scope, locked decisions, primary user flows, UI commitments, UX commitments, acceptance criteria, test/evidence requirements, and exclusions are recorded before implementation judgment.
- Proposal fidelity: the report includes Matches, Divergences, and Ambiguities / Evidence Gaps.
- Output shape: metadata table, lens scorecard, requirement summary, architecture/product/UI/UX/readiness sections, readiness checklist, direct verdict, and report path.
- Evidence discipline: runtime/screenshot/design-reference evidence is used appropriately; `Not Verifiable` stays explicit; `Implemented` is never claimed from inference alone.
- Focus: the assistant stays proposal-scoped and does not expand into unrelated generic code review.

## How to Use

1. Pick a scenario from `evals/scenarios.yaml`.
2. Start a fresh thread and invoke the skill explicitly with the prompt shape in that scenario.
3. Confirm the assistant writes exactly one new versioned report beside the proposal.
4. Inspect the report for:
   - `Platform Scope`
   - `Overall Conformance`, `Overall Readiness`, and `Audit Confidence`
   - `Primary User Flows`
   - `Proposal Fidelity / Divergence`
   - `REQ-*` sections
   - `ARCH-*`, `PROD-*`, `UI-*`, `UX-*`, and `READY-*` findings when relevant
5. Fail the eval if the assistant rewrites the proposal, edits implementation files, invents proposal requirements, or drifts into unrelated repo review.

## Passing Bar

A scenario passes only if:

- the skill is explicitly invoked
- the workflow stays read-only except for the single audit report
- proposal conformance and expert findings are clearly separated
- platform scope is recorded and respected
- divergence and ambiguity are surfaced explicitly
- `Not Verifiable` is preserved when proof is incomplete
- readiness and confidence react appropriately to missing runtime evidence or critical flow gaps
- the agent stays low-thrash and focused on proposal-relevant surfaces
