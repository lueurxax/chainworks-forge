# Lightweight Evals

Use these evals to sanity-check `$proposal-implementation-audit` after edits.

## What to Check

- The skill is used only when explicitly invoked.
- It writes exactly one versioned implementation audit report beside the proposal.
- It keeps the two tracks separate: objective `REQ-*` conformance and routed specialist implementation findings.
- It attempts to reuse prior proposal-review reviewer selection and records whether reuse was exact, partial, delta-based, or rejected.
- It supports Apple, Rust, Go, and cross-stack implementation surfaces without forcing every proposal through Apple UI/UX lenses.
- It distinguishes proposal commitments from specialist findings.
- It distinguishes tests found, tests run, benchmarks found, benchmarks run, runtime evidence, and inference.
- It keeps audits scoped to proposal-relevant implementation surfaces.

## Passing Bar

A scenario passes only if:

- selected reviewers and rejected close alternatives are visible in the report
- every `REQ-*` status cites implementation evidence
- prior proposal-review findings are verified against current implementation rather than trusted blindly
- implementation-only delta risks can add reviewers
- unrelated reviewers are rejected rather than included “just in case”
- `Not Verifiable` remains explicit when proof is incomplete
- readiness can be lower than conformance when runtime/test/rollout/security evidence is insufficient
