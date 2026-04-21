# Pre-Review Evidence Playbook

Use this before routing reviewers.

## Evidence ID scheme

- `DOC-*`: proposal and adjacent documentation.
- `BASE-*`: reusable baseline slices.
- `ART-*`: prior proposal review artifacts.
- `MAP-*`: current code-path, manifest, schema, or runtime ownership facts.
- `DATA-*`: persistence, protocol, payload, schema, or migration facts.
- `AUTH-*`: auth, permission, principal, or trust-boundary facts.
- `OPS-*`: telemetry, rollout, rollback, alerting, deployment, or support facts.
- `PROD-*`: product metric, user segment, scope, or decision checkpoint facts.
- `GAP-*`: missing evidence that blocks confidence.
- `RES-*`: research triggers that cite local evidence.

## Intake order

1. Read the proposal.
2. Read adjacent docs only when they affect the proposed flow.
3. Read `.review-baselines/current-system-baseline.md` when present.
4. Read `<proposal>.review/integration-context.md` when present.
5. Read prior evidence and research packs when relevant.
6. Inspect manifests for actual stack signals.
7. Inspect only the current code slices needed to validate ownership and seams.
8. Build fingerprint tags with evidence IDs.
9. Route reviewers.

## Baseline freshness

Mark a baseline slice `Reused` when it covers the affected stack, entry points, and seams and current code does not contradict it.

Mark it `Partially refreshed` when a narrow code/doc check updated only the affected slice.

Mark it `Stale` when current repo reality contradicts the baseline for a proposal-critical surface.

Mark it `Missing` when the baseline does not cover the affected surface.

Do not refresh unrelated stacks or run the system merely because the baseline is partial.

## Current-code mapping scope

Map enough to answer:

- Which files or modules own the proposed behavior?
- Which manifests prove the stack?
- Which APIs, events, schemas, migrations, flags, queues, workers, or UI entry points are touched?
- Which existing tests or gates are named by the proposal?
- Which current contracts contradict proposal claims?

Stop when routing and findings are evidence-backed. Do not inventory the whole repo by default.

## Evidence gaps

Use an evidence gap instead of guessing when:

- a proposal claims a subsystem but no local source identifies it
- a baseline slice is stale and current code cannot be checked narrowly
- a code path is generated or hidden behind missing artifacts
- a reviewer would be selected only by speculation
- research questions are broad because local evidence is incomplete
