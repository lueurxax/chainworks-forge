# P031 Release Closeout Prep

Status: PREP_ONLY_NOT_SIGNOFF
Owner: P031 release owner
Last Updated: 2026-05-05T19:40:00+03:00

This packet is a preparation checklist for the next P031 audit. It is not release
sign-off evidence and must not be used to mark `proposal-031-readiness` ready
without the run-specific operator evidence listed below.

## Already Closed

- P031 stopped-state implementation audit R8 is recorded in
  `docs/proposals/031-thin-graphql-ui-rewrite_IMPLEMENTATION_AUDIT_R8.md`.
- The canonical `proposal-031` gate passed in audit R8 after the P041 same-tree
  runtime snapshot was current.
- The production P031 GraphQL subscription actor-isolation warning reported in
  audit R8 was removed by making the subscription transport/client/decoder
  boundary explicitly nonisolated.
- Targeted P031 thin GraphQL boundary tests passed after the actor-isolation fix:
  `Proposal031ThinGraphQLReadBoundaryTests`, 61 tests.
- P031 UX/accessibility release sign-off is recorded in
  `docs/evidence/p031-ux-accessibility-signoff.md` after the 2026-05-05
  operator spot check.
- Critical write-path readiness is recorded in
  `docs/evidence/p031-degraded-state-evidence.md` and
  `docs/evidence/p031-dogfood-signoff.md` after the 2026-05-05 release-owner
  confirmation.

## Still Required Before Ready

- Resolve the operator acceptance blocker recorded in
  `docs/evidence/p031-dogfood-signoff.md`. The current UI is not accepted for
  release dogfood because it is not clear enough and differs materially from the
  prior operator workflow.
- Two full-mvp-live dogfood runs completed against the release candidate.
- Operator workflow-completion notes for both dogfood runs.
- Approval diagnostic comprehension evidence on a run with pending approval rows.
- Report payload indicator evidence on representative report artifacts.
- Degraded-state scripted drill evidence, or a dated release-owner waiver for the
  existing partial restart/degraded evidence.
- Freshness confirmation tied to the dogfood run window.

## Gate-Sensitive Evidence Files

The readiness gate intentionally remains strict. Do not edit these files to a
ready status until the corresponding evidence exists:

- `docs/evidence/p031-dogfood-signoff.md`
- `docs/evidence/p031-degraded-state-evidence.md`
- `docs/evidence/p031-freshness-baseline.md`
- `docs/evidence/p031-ux-accessibility-signoff.md`

The current blocker phrases in the remaining open files are expected until the
owner records real dogfood, degraded-state, and freshness evidence.

## Next Audit Capture Table

| Item | Evidence Location | Owner | Status |
| --- | --- | --- | --- |
| Operator UI acceptance | `docs/evidence/p031-dogfood-signoff.md` | P031 release owner | Blocked pending P036/P085 UI work |
| Dogfood run 1 | TBD | P031 release owner | Pending |
| Dogfood run 2 | TBD | P031 release owner | Pending |
| Approval diagnostic check | TBD | P031 release owner | Pending |
| Report payload indicator check | TBD | P031 release owner | Pending |
| VoiceOver/accessibility check | `docs/evidence/p031-ux-accessibility-signoff.md` | P031 release owner | Complete |
| Degraded-state drill or waiver | TBD | P031 release owner | Pending |
| Dogfood freshness confirmation | TBD | P031 release owner | Pending |
| Critical write-path waiver/readiness | `docs/evidence/p031-degraded-state-evidence.md` | P031 release owner | Complete |

## Verification Order

After recording the missing owner evidence, refresh and verify in this order:

1. `./scripts/test-gate.sh proposal-041`
2. `./scripts/test-gate.sh proposal-031`
3. `./scripts/test-gate.sh proposal-072`
4. `./scripts/test-gate.sh proposal-031-readiness`

The first step is required because P031/P072 readiness depends on the P041
same-tree runtime snapshot matching the current commit.
