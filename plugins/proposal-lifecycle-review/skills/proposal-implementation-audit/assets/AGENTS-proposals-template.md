# Proposal Directory Review Instructions

## Proposal Review → Implementation Audit Continuity

When a proposal is reviewed, keep the selected reviewers and evidence pack in `<proposal>.review/`. When the implementation is later audited, `$proposal-implementation-audit` should reuse that selection unless the implementation introduces new stacks, surfaces, or risks.

## Proposal Requirements

Implementation audits will extract `REQ-*` items from explicit commitments. Keep proposal acceptance criteria, non-goals, rollout, telemetry, test, and compatibility commitments concrete.

## Expected Audit Output

Implementation audit reports are written beside the proposal as:

```text
<proposal-stem>_IMPLEMENTATION_AUDIT_R<N>.md
```

Do not hand-edit previous audit reports to represent a new implementation state. Generate a new revision instead.
