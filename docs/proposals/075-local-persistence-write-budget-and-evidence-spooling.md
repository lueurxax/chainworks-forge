# Proposal 075: Local Persistence Write Budget, Evidence Spooling, and SQLite Pressure Control

| Field | Value |
|---|---|
| Date | 2026-04-29 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Rust control plane, SQLite persistence, artifact store, P038 run compaction, P073 stabilization freeze |
| Related | [Provider toolchain cache mapping](../reference/acp-runtime-transport.md#toolchain-cache-mapping), P078 durable side-effect ledger |
| Scope | Introduce write discipline for the local SQLite control-plane database so future durability features, including the side-effect ledger, do not turn SQLite into a high-volume event stream. |
| Goal | Keep SQLite viable as the local canonical state database by routing all writes through a controlled writer, spooling high-volume evidence to files, coalescing non-critical updates, and making write pressure measurable before new durability layers are expanded. |

---

## 1. Why this proposal exists

The system is intentionally local-first:

- one Rust daemon,
- one SQLite database,
- local artifact storage,
- ACP runtimes,
- GraphQL UI projections,
- MCP external control.

That shape is good for simplicity, but it creates one critical constraint:

> SQLite must remain a compact canonical state database, not a runtime event bus.

The current database already stores:

- ideas,
- runs,
- stage executions,
- agent executions,
- approvals,
- artifact metadata,
- work items,
- command journal,
- runtime facts and related execution metadata.

At the same time, runtime behavior is becoming more complex:

- ACP runtime events,
- session lineage,
- proposal-loop artifacts,
- Xcode bridge observations,
- compaction state,
- retry/recovery evidence,
- release side effects.

If every runtime event, tool chunk, transcript update, progress update, telemetry point, and reconciliation step is written directly to SQLite, the system will hit write-lock contention and make local operation worse.

Proposal 075 introduces a local persistence discipline before adding more durability state.

---

## 2. Core decision

SQLite remains the canonical local state database, but it is **not** used for high-volume event streaming.

The system adopts four write classes:

1. **Barrier writes** — must be durable before an external side effect or canonical transition.
2. **Coalesced state writes** — may be merged or delayed briefly.
3. **Evidence spooling** — high-volume evidence goes to files, with SQLite metadata pointers only.
4. **Telemetry rollups** — metrics are summarized before persistence.

---

## 3. Non-goal

This proposal does **not** implement the durable side-effect ledger itself.

That belongs to P078.

Proposal 075 creates the persistence discipline that makes P078 safe to implement without making SQLite worse.

---

## 4. Problem statement

## 4.1 SQLite is single-writer in practice

Even in WAL mode, SQLite allows concurrent readers and one writer. The current code already uses WAL, busy timeout, a file-backed pool, and `BEGIN IMMEDIATE` retry logging.

That is a good foundation, but it means the architecture must be honest:

- multiple concurrent connections do not create multiple concurrent writers;
- write contention must be managed explicitly;
- long-running write transactions are dangerous;
- high-frequency small writes can still starve more important writes.

## 4.2 Runtime systems want to write too often

Without discipline, these become direct DB writes:

- ACP stream chunks,
- tool-call progress,
- stdout/stderr snippets,
- session events,
- retry observations,
- projection invalidations,
- artifact scan updates,
- telemetry counters,
- release settlement evidence.

That is too much for one local control-plane database.

## 4.3 Side-effect ledger can worsen the problem

A durable side-effect ledger is necessary, but if it writes every sub-event directly to SQLite, it becomes another source of contention.

The ledger must write only compact state transitions and durable barriers. Raw evidence must go to files.

---

## 5. Write classes

## 5.1 Class A — Barrier writes

Barrier writes are synchronous and must be durable before the system proceeds.

Examples:

- create side-effect intent,
- mark side-effect executing,
- settle side-effect,
- transition run/stage/agent canonical status,
- record approval decision,
- record command completion,
- enqueue projection invalidation after canonical state change.

Rules:

- barrier writes go through the single `DbWriter`;
- barrier writes must be short;
- barrier writes must not perform network, filesystem scan, or ACP work while holding a transaction;
- barrier writes must be observable through metrics.

## 5.2 Class B — Coalesced state writes

Coalesced writes are useful but not individually critical.

Examples:

- runtime status pings,
- session health summaries,
- stage progress text,
- projection freshness indicators,
- bridge-pool heartbeat summaries.

Rules:

- coalesce by `(run_id, surface, projection_kind)`;
- flush on boundary events;
- flush on short interval;
- drop superseded intermediate values where safe.

## 5.3 Class C — Evidence spooling

High-volume evidence must not be inserted as many SQLite rows.

Examples:

- ACP stream chunks,
- transcripts,
- tool stdout/stderr,
- raw tool traces,
- repeated runtime events,
- model text deltas,
- large delivery readback payloads.

Rules:

- write to files under the artifact/evidence store;
- write compact SQLite metadata:
  - path,
  - checksum,
  - size,
  - kind,
  - run/stage/agent ownership,
  - created_at;
- never insert one SQLite row per stream chunk.

## 5.4 Class D — Telemetry rollups

Telemetry should be aggregated before persistence.

Examples:

- write-lock wait distribution,
- subscription reconnect counts,
- ACP startup latency,
- Xcode bridge pool leases,
- tool-call counts,
- evidence bytes written.

Rules:

- collect in memory or local spool;
- periodically write summary rows;
- do not compete with barrier writes.

---

## 6. `DbWriter` actor

Introduce a single writer actor:

```text
DbWriter
  - owns all write transactions
  - serializes barrier writes
  - batches coalesced writes
  - rejects direct write bypass where possible
  - emits write pressure metrics
```

Readers may continue using the read pool.

Writers should not call `SqlitePool` directly except through the writer abstraction, except in explicitly whitelisted migration/test code.

## 6.1 Priority lanes

`DbWriter` should support priority classes:

1. `critical_barrier`
2. `operator_command`
3. `projection_invalidation`
4. `coalesced_projection`
5. `telemetry_rollup`

Critical barrier writes must not wait behind low-priority telemetry.

## 6.2 Transaction rules

Every write operation must declare:

- operation name,
- write class,
- expected row count,
- whether it is safe to batch,
- whether it is a barrier,
- whether it may be retried after `SQLITE_BUSY`.

---

## 7. Evidence spooling layout

Suggested layout:

```text
{artifact_root}/evidence/
  runs/{run_id}/
    stages/{stage_id}/
      agents/{agent_id}/
        transcripts/
        tool-traces/
        stdout/
        stderr/
        receipts/
        runtime-events/
```

For side effects:

```text
{artifact_root}/effects/
  {effect_id}/
    intent.json
    attempts/
      1.stdout.log
      1.stderr.log
      1.observed-evidence.json
    reconciliation/
      report.json
```

SQLite stores only metadata pointers and summaries.

---

## 8. Projection invalidation policy

Projections should not be rebuilt on every micro-event.

Rules:

- canonical state change creates one invalidation event;
- repeated invalidations for the same projection are coalesced;
- projection materialization can be async;
- UI-facing freshness must remain honest;
- GraphQL subscriptions should emit stable state changes, not noisy internal deltas.

---

## 9. WAL and checkpoint policy

Proposal 075 should add operational metrics and guardrails:

- WAL size,
- checkpoint duration,
- write transaction duration,
- write-lock wait time,
- busy retry count,
- queued write count,
- coalesced writes dropped/merged.

Optional later work:
- manual checkpoint trigger when safe,
- warning when WAL grows beyond threshold,
- warning when long-running readers block checkpoint progress.

---

## 10. Changes to existing code shape

## 10.1 DB layer

Add:

- `DbWriter`
- `WriteClass`
- `WriteOperation`
- `WriteMetrics`
- `EvidenceSpoolRef`

## 10.2 Engine layer

Update services to use `DbWriter` for writes.

High-volume event producers should write evidence files and emit metadata pointers rather than rows.

## 10.3 GraphQL layer

Expose write pressure readbacks:

- DB health,
- projection lag,
- write queue depth,
- WAL size if available,
- recent write-lock wait stats.

## 10.4 MCP layer

Expose optional diagnostics:

- `storage.health`
- `storage.write_pressure`
- `storage.evidence_spool_summary`

---

## 11. Relationship to P078

P078 must follow P075 rules:

- side-effect intent and settlement are barrier writes;
- side-effect attempts may spool stdout/stderr/readback evidence;
- reconciliation reports are artifacts;
- only compact status is stored in SQLite;
- no per-chunk effect event rows.

---

## 12. Acceptance criteria

P075 is complete when:

1. all non-test runtime writes can be routed through `DbWriter`;
2. write classes are implemented and documented;
3. high-volume runtime evidence is spooled to files, not inserted row-by-row;
4. projection invalidations are coalesced;
5. write-lock wait and transaction duration metrics are available;
6. side-effect ledger design can use barrier writes without becoming a high-volume database stream;
7. gates fail if new code writes raw high-volume evidence directly into SQLite.

---

## 13. Tests

Required tests:

- direct write bypass detection where practical;
- evidence spool metadata round-trip;
- coalesced projection invalidation;
- high-volume fake stream produces one artifact pointer, not many rows;
- `DbWriter` priority: critical barrier not starved by telemetry;
- write metrics are emitted for barrier transactions.

---

## 14. Final recommendation

Do not add more durability state until write discipline exists.

SQLite can remain the local canonical state database only if it is protected from becoming a runtime firehose.

This proposal is the guardrail that makes future side-effect settlement safe.
