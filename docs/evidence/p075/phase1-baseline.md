# P075 Baseline — Write-Lock Wait, Busy Retry Rate, Command Latency, WAL Size

> **Status:** same-tree static baseline captured for P075 closeout. Live workload
> latency/WAL numbers remain a canary-promotion input, not an implementation
> prerequisite for the code slice.

**Status: STATIC BASELINE CAPTURED; LIVE CANARY METRICS PENDING**

This file records the same-tree P075 gate and bypass inventory anchor after the manual
closeout slice. Runtime latency metrics require a representative live workload against a
file-backed daemon database; that canary capture is intentionally tracked separately from
the repository implementation truth.

**Ref**: BLOCK-REL-003 (implementation review summary), prepush PPR2-003, audit REQ-011.

---

## Required Metrics

Capture the following metrics under a representative canned workload against a file-backed
SQLite database (not `:memory:`). Record p50 and p95 for latency metrics.

| Metric | Unit | Capture Method | Baseline Value |
|--------|------|----------------|----------------|
| write_lock_wait_p50 | ms | SQLite busy-wait logging (P061 begin_immediate_with_retry) | pending_live_canary |
| write_lock_wait_p95 | ms | SQLite busy-wait logging | pending_live_canary |
| busy_retry_rate | retries/min | Count of BEGIN IMMEDIATE retries per minute | pending_live_canary |
| command_latency_p50 | ms | Time from command enqueue to commit | pending_live_canary |
| command_latency_p95 | ms | Time from command enqueue to commit | pending_live_canary |
| wal_size_bytes | bytes | `PRAGMA wal_checkpoint` or file stat on `-wal` file | pending_live_canary |
| direct_write_call_site_count | count | `./scripts/test-gate.sh proposal-075` inventory output | 3 observed db/src operation literals; 36 allowlisted bypass entries |

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
`expires_after_phase` as the closeout inventory anchor:

```
Phase 2 bypass entries (expires_after_phase=2):  0
Phase 3 bypass entries (expires_after_phase=3):  0
Phase 4 bypass entries (expires_after_phase=4):  0
Phase 5 bypass entries (expires_after_phase=5):  0
Phase 8 bypass entries (expires_after_phase=8):  36
Total temporary_rollout entries:                  31
```

The current closeout model keeps remaining direct-write owners visible as phase-8
allowlist entries. The gate fails on unallowlisted direct write call sites and malformed
entries; each future owner migration should remove or reclassify its entry.

---

_Captured by:_ Codex, P075 manual closeout branch
_Capture date:_ 2026-05-08
_Workload:_ same-tree gate inventory; live canary workload pending
