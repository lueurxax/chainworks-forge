# P031 Freshness Baseline

Status: MEASURED_WITH_DOGFOOD_LIMITATION
Owner: P031 macOS thin UI owner
Blocking Phase: Phase 0d
Blocker Recorded: 2026-04-24
Last Updated: 2026-04-24T20:25:19Z

## Required Measurements

- Representative GraphQL projection freshness p50.
- Representative GraphQL projection freshness p95.
- Targeted read refresh completion behavior under normal daemon conditions.
- Projection lag and stale/unavailable state behavior.

## Local Evidence Status

Live packaged-daemon measurements were produced after restoring the operator database into the packaged daemon path:

- Packaged daemon DB: `~/Library/Application Support/Chainworks Forge/control-plane.db`.
- Source DB copied from: `.chainworks/control-plane.db`.
- Row counts at measurement time: 16 runs, 781 stage executions, 28 approvals, 75,780 artifacts.
- Daemon readiness: `READY`, schema 26, binary schema 26, build `8a0d0494`.
- Evidence JSON: `docs/evidence/p031-runtime/live-graphql-probe-2026-04-24.json`.
- Runtime screenshot: `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-ready-2026-04-24.png`.

Measured authenticated GraphQL read latencies over 25 samples:

| Surface | p50 | p95 | Notes |
| --- | ---: | ---: | --- |
| `daemonStatus` | 2.24 ms | 13.96 ms | Ready response, no GraphQL errors |
| `runs` / Runs Home | 4.25 ms | 14.45 ms | Returned live run rows with `freshnessState=live` |
| `approvalInbox` | 0.82 ms | 1.08 ms | Returned no pending approval rows in current DB |

The live app rendered restored run rows and server freshness badges. This confirms the packaged daemon, GraphQL read transport, run projection read path, and visible `Live` freshness presentation against the restored operator DB.

Limitations:

- This is not the Phase 3 two-run dogfood signoff.
- The measurement did not force a synthetic projection-lag condition; lag/stale behavior remains covered by static/API tests and needs dogfood/degraded-state confirmation.
- Approval diagnostic comprehension was not measured because there were no pending approval rows in the current copied DB.

## Results

Phase 0d freshness baseline is attached for the restored local packaged daemon. Dogfood-specific freshness confirmation remains open for Phase 3.
