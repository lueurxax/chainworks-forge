# P075 Phase 1 Baseline — Write-Lock Wait, Busy Retry Rate, Command Latency, WAL Size

> **⛔ BLOCKER: capture pending — Phase 2 must not begin until this file is populated.**
>
> An operator must run the canned workload described below, fill all `_TO FILL_` values,
> and commit this file. Phase 2 canary promotion criteria are subjective without a comparison anchor.
> Tracked as BLOCK-REL-003 in run `4aeb45a9-b5e1-4891-b08a-60d994204083`.

**Status: REQUIRES HUMAN OPERATOR CAPTURE**

This file is a tracked artifact placeholder for the P075 Phase 1 baseline capture required
by `approved_proposal` line 548 and Phase 1 scope (line 570). It must be populated before
Phase 2 canary work begins so that Phase 2 numeric thresholds can be validated against a
comparison anchor.

**Ref**: BLOCK-REL-003 (implementation review summary), prepush PPR2-003, audit REQ-011.

---

## Required Metrics

Capture the following metrics under a representative canned workload against a file-backed
SQLite database (not `:memory:`). Record p50 and p95 for latency metrics.

| Metric | Unit | Capture Method | Baseline Value |
|--------|------|----------------|----------------|
| write_lock_wait_p50 | ms | SQLite busy-wait logging (P061 begin_immediate_with_retry) | _TO FILL_ |
| write_lock_wait_p95 | ms | SQLite busy-wait logging | _TO FILL_ |
| busy_retry_rate | retries/min | Count of BEGIN IMMEDIATE retries per minute | _TO FILL_ |
| command_latency_p50 | ms | Time from command enqueue to commit | _TO FILL_ |
| command_latency_p95 | ms | Time from command enqueue to commit | _TO FILL_ |
| wal_size_bytes | bytes | `PRAGMA wal_checkpoint` or file stat on `-wal` file | _TO FILL_ |
| direct_write_call_site_count | count | `./scripts/test-gate.sh proposal-075` inventory output | _TO FILL_ |

## Capture Protocol

1. Start the daemon with a file-backed SQLite database:
   ```bash
   DATABASE_URL="sqlite:///path/to/test.db?mode=rwc" \
   GRAPHQL_ADDR="127.0.0.1:4000" \
   RUST_LOG=info,db=debug \
   ./target/debug/control-plane 2>/tmp/cw-baseline.log &
   ```
2. Run a representative canned workload (e.g., 3-5 runs with the full-mvp-live workflow).
3. Extract metrics from `/tmp/cw-baseline.log` and the GraphQL `schedulerHealthSummary`
   and `dbWriterContentionSummary` endpoints.
4. Record WAL size: `ls -la /path/to/test.db-wal`.
5. Run `./scripts/test-gate.sh proposal-075` to get the direct-write inventory count.
6. Fill in the table above and commit this file before Phase 2 work begins.

## Threshold Guidance (from proposal)

Initial warn/critical thresholds to compare against captured baseline:
- `write_lock_wait_p95_ms`: warn=100, critical=500
- `busy_retry_rate_per_minute`: warn=5, critical=30
- `wal_size_bytes`: warn=134217728 (128 MiB), critical=536870912 (512 MiB)
- `class_a_transaction_duration_p95_ms`: warn=50, critical=200

If the captured baseline is already above a warn threshold, record the finding here and
tighten or loosen the thresholds with rationale in `open_questions` resolution before
Phase 2 wires any new write path.

## Direct-Write Inventory Snapshot

Run `./scripts/test-gate.sh proposal-075` and record the bypass entry counts by
`expires_after_phase` as the Phase 1 inventory anchor:

```
Phase 2 bypass entries (expires_after_phase=2):  _TO FILL_
Phase 3 bypass entries (expires_after_phase=3):  _TO FILL_
Phase 4 bypass entries (expires_after_phase=4):  _TO FILL_
Phase 5 bypass entries (expires_after_phase=5):  _TO FILL_
Total temporary_rollout entries:                  _TO FILL_
```

Each phase must reduce the count strictly. Phase 7 fail-closed gate will assert count=0.

---

_Captured by:_ (operator name / run id)
_Capture date:_ (ISO 8601 date)
_Workload:_ (description of canned workload used)
