# MVP Sign-Off Proof

Current implementation and proof status for the MVP hardening and sign-off layer that was previously tracked through Proposal 008 reviews and implementation audits.

## Status

| Field | Value |
|---|---|
| Slice | MVP Sign-Off |
| Source contract | [../reference/mvp-sign-off.md](../reference/mvp-sign-off.md) |
| Current implementation status | Implemented |
| Current readiness | Ready with Risks |
| Primary evidence owner | approved-host current-head benchmark, export, and UI proof artifacts |
| Last consolidated audit | `R16` on `2026-03-29` |

## What is considered proven

The current proof set supports these claims:

- benchmark and sign-off state persist outside the operational `Run` aggregate,
- the evaluator produces replayable `GO/HOLD` decision snapshots from persisted benchmark records,
- completed-run export and MVP sign-off summary remain shell-owned subordinate routes,
- approval relaunch continuity, recovery, run-progress, and export states are covered by current-head proof paths,
- the approved-host non-UI proving path is green on current head,
- the approved-host screenshot-bearing UI smoke path is green on current head,
- current-head app-launched happy-path and non-happy-path repo-backed evidence packets exist and include the required delivery receipts.

## Current-head proof set

The accepted current-head proof story now rests on four pillars:

1. current-head repo-backed happy-path dogfood evidence,
2. current-head repo-backed non-happy-path dogfood evidence,
3. green approved-host `fast` gate,
4. green approved-host screenshot-bearing `ui-smoke` gate.

### Approved-host gate results

Accepted remote gate artifacts:

- `fast` bundle from the approved host on current head,
- `ui-smoke` bundle from the approved host on current head,
- screenshot-bearing export-hub coverage inside that UI smoke path.

These prove the canonical non-UI and shell-level UI sign-off routes without relying on local UI execution.

### Current-head repo-backed dogfood artifacts

Accepted current-head app-launched repo-backed runs:

- one happy-path run with terminal `completed`,
- one non-happy-path run with terminal `blocked`,
- both on the same current proving tree,
- both with run-storage artifacts and exported evidence packets.

Required artifact expectations are met:

- happy-path contains `delivery_receipt`, `release_manifest`, `git_push_receipt`, `connect_upload_receipt`, and `release_bundle_manifest`,
- non-happy-path contains `delivery_receipt`, `release_manifest`, and `git_push_receipt`,
- non-happy-path intentionally does not contain `connect_upload_receipt`.

## What remains risky

The remaining risk is no longer contractual.
It is operational:

- the strongest proof path depends on the approved remote host being available and correctly configured,
- some evidence remains environment-specific rather than trivially replayable from every audit environment,
- later heads must be reproved rather than inheriting current-head sign-off by assumption.

That does not reopen the MVP sign-off contract itself.
It narrows how far one current-head proof bundle should be generalized without rerun.

## Recommended usage

Use:

- [../reference/mvp-sign-off.md](../reference/mvp-sign-off.md) for the stable sign-off contract,
- [../reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md) for the repo-backed delivery slice that sign-off evaluates,
- [full-mvp-delivery-proof.md](full-mvp-delivery-proof.md) for the narrower delivery-slice proof story.

Do not treat historical proposal-specific audits as the primary source anymore.
Those were transitional records on the path to the stable reference and evidence pair above.
