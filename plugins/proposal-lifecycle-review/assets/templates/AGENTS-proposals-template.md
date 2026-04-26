# Proposal Area Guidance

Proposal review artifacts should live beside the proposal in `<proposal>.review/` when practical.

Recommended files:

- `evidence-pack.md`
- `final-review.md`
- `research-pack.md`
- `reviewer-selection.yaml`
- `integration-context.md`

Review workflow:

1. Read the proposal first.
2. Read adjacent docs and baseline slices only as needed.
3. Fingerprint stack, surface, and risk tags before selecting reviewers.
4. Record selected reviewers, rejected close alternatives, evidence ids, and routing rationale.
5. Preserve `reviewer-selection.yaml` so implementation audits can reuse the reviewer set.

Audit workflow:

1. Read the proposal and prior review artifacts.
2. Reuse `reviewer-selection.yaml` when it still matches the implementation.
3. Reroute only when prior selection is stale or the user asks for reroute mode.
4. Add delta reviewers only from implementation evidence.
5. Write one versioned implementation audit report beside the proposal.
