# P031 Dogfood Sign-Off Template

Status: READY_TEMPLATE_WITH_RUNTIME_PREREQS_ATTACHED
Owner: P031 release owner
Blocking Phase: Phase 3
Template Date: 2026-04-24
Last Updated: 2026-05-05T19:40:00+03:00

## Phase 3 Checklist
- [ ] Run prerequisite gates (P027, P041, P042, P043, P031).
- [ ] Two full-mvp-live dogfood runs completed.
- [ ] Operator workflow-completion notes captured.
- [ ] Degraded-state recovery and approval diagnostic evidence captured.
- [x] UX/Accessibility spot check completed.
- [ ] Freshness baseline measured (p50/p95).
- [ ] Degraded-state/fail-closed evidence or waiver attached.
- [x] Critical write-path readiness or release-owner waiver confirmed.

## Local Evidence Status

This artifact is the Phase 3 sign-off template required before dogfood start. It is not dogfood completion evidence.

Attached pre-dogfood evidence:

- Live packaged daemon restored to the operator DB and serving GraphQL read projections.
- Freshness baseline attached in `docs/evidence/p031-freshness-baseline.md`.
- Runtime screenshots attached under `docs/evidence/p031-runtime/`.
- Partial degraded-state restart evidence attached in `docs/evidence/p031-degraded-state-evidence.md`.
- Report payload metadata-only evidence attached in `docs/evidence/p031-runtime/report-payload-live-evidence-2026-04-25.json`.
- UX/accessibility human spot check sign-off attached in `docs/evidence/p031-ux-accessibility-signoff.md`.
- Critical write-path readiness was confirmed by the release owner on 2026-05-05: the P031 macOS UI remains a GraphQL read surface, local workflow/control writes remain unavailable from the UI, and command/control operations stay outside the thin UI except for approved approval-only GraphQL mutations.
- Current copied DB contains no completed `Full MVP Live` runs; all current run rows are `blocked` or `cancelled`, so this tree cannot honestly claim the two-run dogfood completion criterion from historical data.

Still required for Phase 3 sign-off:

- Two full-mvp-live dogfood runs.
- Operator workflow-completion notes.
- Approval diagnostic comprehension on a run with pending approval rows.
- Report payload indicator evidence on representative report artifacts.
- Release-owner acceptance/waiver for degraded-state evidence if no scripted drill is run.
- Operator acceptance of the current workflow UI after the P036/P085 UI restoration work is complete.

## Operator Acceptance Blocker

The operator explicitly did not accept the current P031 thin UI for release dogfood on 2026-05-05.

Operator feedback:

- The workflow is not clear enough for release acceptance.
- The current UI differs materially from the prior operator workflow.
- The relevant UI concerns are tracked in the UI follow-up proposals, especially P036 for visual/navigation restoration and P085 for thin-client affordance parity.

This blocks Phase 3 dogfood sign-off. The two required `full-mvp-live` dogfood runs should not be treated as release acceptance until the UI restoration/affordance concerns have been addressed or explicitly waived by the release owner.

## Sign-Off

Not signed for full dogfood closeout. Complete the remaining run-specific dogfood, degraded-state, and freshness evidence before Phase 3 closeout.
