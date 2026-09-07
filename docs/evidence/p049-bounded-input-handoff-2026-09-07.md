# P049 Bounded Input Handoff: Proof and Recovery Conditions

Date: 2026-09-07. Base HEAD: `57ada331d7635d9fdf106fafcba9bc82a748b09f`.
Scope: the approved bounded production slice, not all P049 strategy tools.

## Evidence and Limits

Read-only P095 diagnosis found a 101,202-byte UTF-8 persisted invocation prompt
before runtime contract additions for run `bd83a310-360f-4d45-b4c6-a94873de3733`.
The observed provider timeout/transport closure does not prove that input size
was its sole cause. The synthetic reproducer failed on the old builder at
111,694 prompt bytes; it now stays below 65,536 bytes while the snapshot retains
every source byte, including the final required fact, with matching SHA-256.

The implementation preserves mandatory system/mission/skill/output authority,
uses a bounded aggregate inline budget, and hands off larger inputs as sealed,
content-addressed run-owned files. Final admission includes late runtime and
retry additions. Repair and continuation retain the typed input binding;
resurrection validates the actual continuation before launching its process.
Input failures cannot consume historical escalation triggers or silently retry.

Parent review identified that targeted retry still copied historical oversized
InvokeAgent prompts. The same slice now rehydrates those from verified frozen
task authority before the retry transaction. The regression first failed with
a 115,227-byte queued prompt; another failed at 66,708 bytes after current
backlog context pushed a previously bounded prompt over the limit. Both cases
now rebuild through the manifest-backed builder. The unchanged original work
item remains evidence, and missing or inconsistent reconstruction authority
rejects the command without a new attempt or queued provider invocation.

See [the implemented contract](../reference/acp-runtime-transport.md#bounded-input-handoff-p049-slice)
for limits, provenance rules, missing-source compatibility, and the explicit
same-OS-user mutation limitation. This is preflight tamper detection plus
no-overwrite publication, not a lifetime sandbox against a hostile same-UID
process. Provider reasoning over all input bytes is not independently proven.

## Verification

Commands below use `../scripts/cargo-managed` from `control-plane/`.

| Check | Result |
|---|---|
| `test -p acp --lib --test input_pressure --quiet -- --test-threads=1` | 196 unit + 15 input-pressure tests passed |
| `test -p engine --lib p049_` | 7 passed |
| `test -p engine --test integration p049_ -- --test-threads=1` | 11 passed, including all 6 targeted-rehydration tests |
| `test -p engine --test integration p049_targeted_retry_ -- --test-threads=1` | 6 passed after review hardening, including all 3 pre-transaction rejection regressions |
| `test -p engine --test integration targeted_retry -- --test-threads=1` | 8 passed, 9 old snapshot-fixture failures listed below |
| `test -p daemon --test proposal_086_mcp_continuation_live_reuse -- --test-threads=1` | 7 passed, including missing-snapshot/no-process-launch proof |
| `test -p engine --lib proposal_writer_` | 6 passed |
| `test -p engine --lib agent_context_` | 7 passed |
| `check --workspace --all-targets` | Passed; existing DB/GraphQL/MCP warnings remain |
| Repository `./scripts/test-gate.sh guardrails` | Passed; sandbox denied an optional `ps` probe; boundary guard excludes uncommitted changes |
| Repository `./scripts/test-gate.sh build` | Passed after targeted-retry extension; final embedded daemon includes pre-transaction final-budget validation; not installed or launched |
| Scoped `rustfmt --check`; `git diff --check` | Passed |

Final compile-only build log: `/tmp/chainworks-p049-targeted-retry-build-20260907.log`.
The embedded daemon was checked in the gate's `build-20260907-135735-DerivedData`
output for `input_context_retry_final_prompt_too_large`. This confirms inclusion
of the targeted-retry final-budget guard in that build product, not deployment
provenance.

Regression tests cover UTF-8 byte accounting, exact cap admission, aggregate
inline budget, full content read through a real ACP subprocess fixture, source
replacement, missing/same-size-tampered/writable snapshots, symlink escapes,
writable directories, FIFO sources, cross-run/path substitution, binding
serialization/disagreement, and final-contract overflow. Engine tests assert
durable failure and zero retry/fallback after `AdvanceRun`, including a
diagnostic persistence failure. Acquisition failures durably block empty stages.

Targeted rehydration covers the historical oversized prompt and backlog-induced
overflow. It proves the old prompt cannot reach the ACP adapter, the bounded
replacement can, full snapshot bytes and their final required fact survive,
frozen system/output requirements and current approval/backlog authority remain
inline, and retry metadata/operator-instruction bindings survive. Six rejection
cases cover missing source, unknown task, missing frozen plan, oversized
mandatory instructions, an existing manifest binding, and mismatched inputs;
each leaves the original blocked run and work item unchanged with no new attempt.

Independent review found dynamic aggregation without a `p060_*` marker could
lose selected-reviewer authority, and final additions could exceed admission
after retry state was committed. Three additional tests reproduced these gaps
before the fix and now pass: dynamic aggregation, exhausted mandatory-context
runtime reserve, and final backlog overflow. Rehydration now rejects unsupported
aggregation, preserves 16 KiB runtime reserve, and measures projected late
context with the same executor instruction/contract renderers before commit.
The follow-up static review confirmed both findings resolved with no new
actionable correctness gaps in this delta. The 7 continuation/resurrection
fixtures were rerun successfully after these changes.

The broader targeted-retry sweep has nine unchanged-from-HEAD fixture failures:
the eight `test_targeted_retry_falls_back_from_*` cases for Claude aggregation,
Claude reviewer, Claude security checker, Codex reviewer, Codex writer, Gemini
docs guardian, Gemini reviewer, and Junie code writer, plus
`test_targeted_retry_uses_current_catalog_binding_when_agent_profile_changed`.
All fail on `frozen_snapshot_contract_incompatible` because the fixture stores
an incomplete snapshot JSON/hash quartet. This is the existing entry preflight,
before the new rehydration branch. Their test bodies were compared byte-for-byte
with HEAD and are unchanged; this extension does not fix those fixtures.

Broader engine unit sweep: **561 passed, 4 failed** at the time of that sweep.
Their test bodies are unchanged from base HEAD; they were not fixed by this slice:

- `proposal_053_settlement_boundary_records_idempotency_key`: expects a materialized file after the settlement-only boundary; also fails in isolation.
- `provider_quota_runtime_receipt_preserves_explicit_claude_reset_time`: expected August 30 but observed September 7 reset date.
- `targeted_p058_retry_refreshes_stale_tier_from_the_durable_ledger`: invalid fixture `idea_id` parsing.
- `persisted_dynamic_health_fallback_payload_uses_frozen_target_profile_authority`: fixture model differs from frozen authority in the model-refresh working tree.

The parallel ACP unit sweep had one process-environment/sccache fixture failure;
the isolated test and all 196 tests run sequentially passed. No full-suite-green
claim is made. No real provider acceptance run or local UI test was performed.

## Exact P049 Files

The targeted-retry follow-up changes only these six files from the initial slice:

```text
control-plane/crates/engine/src/command_handler.rs
control-plane/crates/engine/src/orchestrator.rs
control-plane/crates/engine/src/executor.rs
control-plane/crates/engine/tests/integration.rs
docs/reference/acp-runtime-transport.md
docs/evidence/p049-bounded-input-handoff-2026-09-07.md
```

Behavior, focused tests, and documentation:

```text
control-plane/crates/acp/src/input_context.rs
control-plane/crates/acp/src/lib.rs
control-plane/crates/acp/src/manager.rs
control-plane/crates/acp/tests/input_pressure.rs
control-plane/crates/engine/src/executor.rs
control-plane/crates/engine/src/command_handler.rs
control-plane/crates/engine/src/orchestrator.rs
control-plane/crates/engine/tests/integration.rs
control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs
docs/proposals/049-context-strategy-management-mcp-tools.md
docs/reference/acp-runtime-transport.md
docs/evidence/p049-bounded-input-handoff-2026-09-07.md
```

Compatibility-only request literal updates (`input_manifest: None`):

```text
control-plane/crates/acp/src/adapters/claude.rs
control-plane/crates/acp/src/adapters/codex.rs
control-plane/crates/acp/src/adapters/gemini.rs
control-plane/crates/acp/src/adapters/junie.rs
control-plane/crates/acp/src/adapters/mod.rs
control-plane/crates/acp/src/session.rs
control-plane/crates/acp/src/transport.rs
control-plane/crates/acp/tests/integration.rs
control-plane/crates/daemon/src/xcode_broker_http.rs
control-plane/crates/engine/examples/p089_acp_live_canary.rs
```

The pre-existing 16-file model-refresh work was retained. Its catalog hash is
`639c27d3d36d86659dfa23235537561c974b7ca48c7e5b17c2eab62d3ace3449`;
matrix hash is `e2e78059d4d03936d4c88e0009435b8b9540611cd2fb1c08707004bd154d26b0`.
No model policy, catalog binding, DB migration, or strategy MCP tool was added
or changed by P049. Two already-dirty ACP files also require the additive request
literal field above; their prior model changes remain intact.

## Deployment and P095 Retry

No app/daemon deployment or restart, P095 retry, P070 lifecycle change, commit,
or push was performed. Build products are not evidence of a running deployment.

1. Do not replace the app-owned daemon while P070
   `3ebb7bfd-200f-4b5a-bb09-bda6bf96c770` is active. Recheck live run state,
   active agents, and leased/running work before any authorized maintenance.
   Do not act on the separate old P070 human gate
   `5130f641-768f-45e9-8562-015367637657`.
2. Once explicitly authorized and quiescent, build and deploy the matching app,
   daemon, catalog, and model matrix using the normal supported deployment path.
   Verify executable provenance, signature, readiness, and post-deploy version;
   do not start a second daemon against another database or write lifecycle SQL.
3. Recheck P095's frozen snapshot, current backlog, source readability, and run
   ownership. Keep its frozen provider/model authority unchanged. This change
   does not retrospectively rewrite queued oversized prompts.
4. Use one authorized retry for `state_5_proposal_refined`. A targeted retry
   with the exact source `agent_execution_id` now rebuilds a historical
   oversized flat prompt from frozen task authority before enqueueing. Verify
   `p049_prompt_rehydration` and the replacement manifest in the new work item.
   A full-stage retry also rebuilds via `AdvanceRun`, but is no longer required
   solely to avoid copying P095's historical oversized prompt. Do not bypass a
   rehydration rejection by changing authority or omitting required sources.
5. Read back the new stage attempt/work item, manifest, digests, final admission,
   provider receipt, and output-contract result. An idempotent replay is not a
   new attempt. Stop on another input failure or timeout; do not loop retries or
   assume that bounded input has solved all provider/runtime failure causes.
