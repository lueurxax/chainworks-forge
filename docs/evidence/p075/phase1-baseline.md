# P075 Baseline — Write-Lock Wait, Busy Retry Rate, Command Latency, WAL Size

> **Status:** same-tree numeric baseline plus file-backed storage canary captured
> for P075 closeout. The next promotion step can still add a larger daemon
> workload, but the repository gate now proves non-null live lock/WAL/writer
> readback on a file-backed SQLite database.

**Status: NUMERIC BASELINE AND FILE-BACKED CANARY CAPTURED**

This file records the same-tree P075 gate, bypass inventory anchor, and the
file-backed storage canary added for the manual closeout slice. The canary uses a
real SQLite WAL file and submits multiple Class A writes through the shared
`DbWriter`, then asserts that `storageHealth` exposes non-null lock wait,
transaction-duration, heartbeat/drain, and WAL fields. The same gate now also
proves the Class D write-pressure rollup producer writes a bounded snapshot and
purges history to the 24-hour/latest-288 retention contract.

**Ref**: BLOCK-REL-003 (implementation review summary), prepush PPR2-003, audit REQ-011.

---

## Required Metrics

Capture the following metrics under a representative canned workload against a file-backed
SQLite database (not `:memory:`). Record p50 and p95 for latency metrics.

| Metric | Unit | Capture Method | Baseline Value |
|--------|------|----------------|----------------|
| write_lock_wait_p50 | ms | `storage_health_file_backed_canary_reports_lock_wal_and_writer_metrics` via DbWriter-owned P061 lock metrics | 0 |
| write_lock_wait_p95 | ms | same canary | 1 |
| busy_retry_rate | retries/min | same canary, uncontended file-backed workload | 0.0 |
| command_latency_p50 | ms | `storageHealth.writer.transactionDurationP50Ms` from DbWriter transaction accounting | 0 |
| command_latency_p95 | ms | `storageHealth.writer.transactionDurationP95Ms` from DbWriter transaction accounting | 2 |
| wal_size_bytes | bytes | file stat on the canary SQLite `-wal` file through `storageHealth.wal.sizeBytes` | 45352 |
| telemetry_rollup_retention_limit | windows | `class_d_rollup_producer_persists_bounded_snapshot_and_purges_retention` | 288 |
| direct_write_call_site_count | count | `./scripts/test-gate.sh proposal-075` inventory output | 0 production runtime direct transaction sites; 5 permanent infrastructure bypass entries; 0 temporary rollout bypass entries |

## Capture Protocol

1. Run the gate-backed file canary:
   ```bash
   cd control-plane
   cargo test -p db storage_health_file_backed_canary_reports_lock_wal_and_writer_metrics -- --nocapture
   ```
2. For a larger operational canary, start the daemon with a file-backed SQLite database:
   ```bash
   DATABASE_URL="sqlite:///path/to/test.db?mode=rwc" \
   GRAPHQL_ADDR="127.0.0.1:4000" \
   RUST_LOG=info,db=debug \
   ./target/debug/control-plane 2>/tmp/cw-baseline.log &
   ```
3. Run a representative canned workload (e.g., 3-5 runs with the full-mvp-live workflow).
4. Extract metrics from `/tmp/cw-baseline.log` and the GraphQL `schedulerHealthSummary`
   and `dbWriterContentionSummary` endpoints.
5. Record WAL size: `ls -la /path/to/test.db-wal`.
6. Run `./scripts/test-gate.sh proposal-075` to get the direct-write inventory count.
7. Commit any broader operational canary evidence under `docs/evidence/p075/`.

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
Phase 8 bypass entries (expires_after_phase=8):  5
Total temporary_rollout entries:                  0
```

The Phase 8 closeout model has retired all temporary rollout bypasses. The
remaining allowlist entries are permanent infrastructure scopes only: migrations,
tests, startup repair, and evidence-spool orphan repair. The gate now fails on any
`temporary_rollout` row and on production runtime direct SQL writes that bypass
DbWriter.

## Captured Numeric Sample

Captured with:

```bash
cd control-plane
cargo test -p db storage_health_file_backed_canary_reports_lock_wal_and_writer_metrics -- --nocapture
```

Sample timestamp: `2026-05-09T08:28:21.083580+00:00`

| Readback Field | Value |
|----------------|-------|
| `storageHealth.writer.writeLockWaitP50Ms` | `0` |
| `storageHealth.writer.writeLockWaitP95Ms` | `1` |
| `storageHealth.writer.transactionDurationP50Ms` | `0` |
| `storageHealth.writer.transactionDurationP95Ms` | `2` |
| `storageHealth.writer.busyRetryRatePerMinute` | `0.0` |
| `storageHealth.writer.busyRetryExhaustedTotal` | `0` |
| `storageHealth.wal.sizeBytes` | `45352` |

---

_Captured by:_ Codex, P075 manual closeout branch
_Capture date:_ 2026-05-08
_Workload:_ same-tree gate inventory and file-backed DbWriter/WAL canary
