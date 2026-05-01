# Proposal 066: Provider Toolchain Cache Mapping

| Field | Value |
|---|---|
| Date | 2026-04-30 |
| Status | Implementation-ready proposal (R4) |
| Author | Andrey Khasanov |
| Source revision | `066-rewrite-2026-04-30-r4` |
| Replaces | Earlier P066 draft and rewrite draft |
| Depends on | Rust control-plane provider runtime homes and toolchain caches; Xcode MCP bridge pool |
| Related | P075 Local Persistence Write Budget; P078 Durable Side-Effect Ledger |
| Scope | Add adapter-local cache/build directory mappings for Swift/Xcode and Go provider sessions under the generic `TOOLCHAIN_HOME` contract. |
| Goal | Give daemon-launched provider sessions writable, isolated, observable toolchain caches without changing scheduler semantics, workflow semantics, release settlement semantics, or the local persistence model. |

---

## 1. Summary

P066 makes provider-launched Swift/Xcode and Go toolchain work use isolated writable cache roots under `TOOLCHAIN_HOME`.

It deliberately stays inside the provider/toolchain boundary. It does not solve release settlement, duplicate-push prevention, SQLite write pressure, workflow transitions, or operator control surfaces. Those concerns are owned by P078, P075, existing workflow proposals, and P072 respectively.

The R4 revision freezes the previously drifting design:

- `CHAINWORKS_TOOLCHAIN_HOME` and `TOOLCHAIN_HOME` remain the generic invariant.
- A versioned `agents[].toolchain_cache_policy` catalog contract controls opt-in behavior.
- Frozen catalog/run-plan snapshots fail closed when they contain unsupported P066 policy shapes.
- Runtime diagnostics are authoritative from frozen `runplan_snapshot` truth, not live catalog state.
- Swift read paths consume the contract through `ToolchainMappingReadAdapter`.
- Xcode queue wait, mapping setup, setup failure, crash quarantine, and cleanup telemetry are separate contracts.
- P066 does not introduce release side-effect settlement or high-volume SQLite writes.

## 2. Why this rewrite exists

The original P066 was a focused provider toolchain cache mapping proposal. During implementation and review, a deeper issue surfaced: external release side effects can complete while the control plane fails before durable settlement.

That issue is real, but it is not P066.

- P078 owns durable side-effect intent, settlement, reconciliation, and retry blocking.
- P075 owns SQLite write-budget discipline, evidence spooling, and high-volume persistence pressure.
- P066 owns safe, isolated, observable toolchain cache roots for provider sessions.

Keeping that boundary matters because cache isolation is ready to implement independently, while release settlement and persistence budgeting require separate durable architecture.

## 3. Problem

Daemon-launched provider sessions already receive a generic writable toolchain root, but Swift/Xcode and Go tools can still write generated output into unsafe locations:

- read-only provider runtime homes;
- host-global DerivedData and SwiftPM caches;
- operator-global Go caches;
- repo-global generated build outputs;
- shared directories that allow cross-run contamination.

The practical result is avoidable filesystem failures, slow or stale builds, cross-run contamination, and operator readback that cannot distinguish a model/tool failure from missing writable toolchain roots.

## 4. Goals

- Give Swift/Xcode executions isolated writable cache roots under the existing `TOOLCHAIN_HOME` contract.
- Give Go executions isolated writable `GOCACHE`, `GOMODCACHE`, `GOPATH`, and `TMPDIR`.
- Carry Xcode cache mapping through the real host-executor path, not only provider-local environment shaping.
- Freeze one versioned configuration surface for cache scope.
- Expose bounded typed diagnostics for mapping decisions, queueing, setup failures, cleanup ownership, and legacy sentinel states.
- Preserve language-neutral scheduler semantics.
- Keep rollback and historical readback semantics explicit for snapshot-backed runs.

## 5. Non-Goals

P066 does not include:

- release side-effect settlement, reconciliation, retry blocking, or duplicate-side-effect prevention;
- SQLite write-budget redesign, evidence spooling redesign, or transcript persistence changes;
- workflow transition-semantic changes;
- scheduler language-awareness or new scheduler capacity dimensions;
- GraphQL mutations, MCP control tools, or operator UI write workflows;
- simulator state virtualization, Xcode consent handling, or Xcode MCP bridge pooling behavior;
- a promise that every standalone Swift tool has complete cache redirection.

When unsupported Swift tool behavior is discovered, P066 must surface it explicitly rather than pretending the mapping is complete.

## 6. Catalog Contract

P066 introduces one additive agent-catalog surface:

```yaml
agents:
  example_agent:
    toolchain_cache_policy:
      version: 1
      enabled: true
      xcode_scope: run
      go_scope: session
```

### 6.1 Schema

`agents[].toolchain_cache_policy` is a versioned object with exact keys:

- `version`: required integer, currently `1`;
- `enabled`: required boolean;
- `xcode_scope`: optional enum, `run` or `session`;
- `go_scope`: optional enum, `run` or `session`.

Defaults:

- missing block means `policy_absent`;
- `enabled: false` disables mapping and ignores family scopes;
- when enabled, omitted `xcode_scope` defaults to `run`;
- when enabled, omitted `go_scope` defaults to `session`.

Unknown keys and unknown enum values fail workflow compilation. There is no workflow-level override and no scheduler-owned interpretation.

### 6.2 Frozen Snapshot Compatibility

Frozen catalog and run-plan snapshots that carry `toolchain_cache_policy` must also carry top-level format-version gates.

Required fields:

- `catalog_snapshot_format_version`;
- `run_plan_snapshot_format_version`.

Rules:

- pre-P066 snapshots that omit both the version field and `toolchain_cache_policy` decode as `legacy_v0` and surface `policy_absent`;
- snapshots that contain `toolchain_cache_policy` but omit the top-level version fail as `frozen_snapshot_contract_incompatible`;
- snapshots that contain a higher unsupported version fail deterministically;
- readers must not silently drop the field and reinterpret the execution as `policy_absent`.

This applies to both Rust and Swift readers.

### 6.3 Swift Read Adapter

Operator-facing Swift consumers must decode this contract through `ToolchainMappingReadAdapter`.

The adapter owns:

- frozen-snapshot compatibility checks;
- `toolchain_cache_policy` decoding;
- `actualToolchainMappingDiagnostics` decoding;
- legacy sentinel synthesis;
- enum handling;
- redaction before rendering.

Known Swift consumers that must not directly decode this contract with `try?` fallback after Phase 0:

- `Chainworks Forge/Engine/RunPlanCompiler.swift`;
- `Chainworks Forge/Engine/ExecutionService.swift`;
- `Chainworks Forge/Engine/RunReportBuilder.swift`;
- `Chainworks Forge/Engine/RunComparisonService.swift`.

## 7. Root Layout

The generic roots remain:

- `CHAINWORKS_TOOLCHAIN_HOME`;
- `TOOLCHAIN_HOME`.

P066 maps tool-specific paths under provider/session or provider/run scoped subtrees:

```text
{TOOLCHAIN_HOME}/
  providers/
    {mapping_family}/
      {scope_key}/
        xcode/
          DerivedData/
          ModuleCache.noindex/
          SDKStatCaches/
          SourcePackages/
          tmp/
        swift/
          build-cache/
          package-cache/
          module-cache/
          tmp/
        go/
          build-cache/
          module-cache/
          gopath/
          tmp/
```

Default scoping:

- Xcode: `run`, because DerivedData and host-executed Xcode work need same-run coordination;
- Go: `session`, because standard Go cache isolation is cheaper and less coupled to host execution.

Session scope is safer for cleanup. Run scope is used only where incremental reuse or host-executor behavior requires it.

## 8. Xcode and Swift Mapping

The adapter/host-executor boundary creates and exposes:

- `CHAINWORKS_XCODE_DERIVED_DATA_DIR`;
- `CHAINWORKS_XCODE_MODULE_CACHE_DIR`;
- `CHAINWORKS_XCODE_SDK_STAT_CACHE_DIR`;
- `CHAINWORKS_XCODE_SOURCE_PACKAGES_DIR`;
- `TMPDIR`.

For `xcodebuild`, explicit arguments are preferred:

```text
-derivedDataPath <.../DerivedData>
-clonedSourcePackagesDirPath <.../SourcePackages>
```

Host-executed Xcode work must derive `TMPDIR` from prepared mapping state rather than trusting arbitrary provider-supplied environment values.

Standalone SwiftPM/lower-level Swift tools use supported environment variables where possible. Unsupported mappings are recorded in diagnostics as unsupported, not treated as successful redirection.

## 9. Same-Run Xcode Concurrency

When `xcode_scope=run`, host-executed Xcode work serializes per run.

Queue wait is not setup time:

- queue wait uses a bounded `300000 ms` deadline or a smaller remaining execution budget;
- queue timeout is classified as `xcode_run_scope_queue_timeout`;
- directory preparation failure is classified as `toolchain_mapping_setup_failed`;
- readback surfaces queue status and queue wait separately from active build duration.

Crash restart must either prove the run-scoped Xcode root is safe to reuse or quarantine the old root before creating a fresh one.

## 10. Go Mapping

The adapter creates and exposes:

- `GOCACHE=<.../build-cache>`;
- `GOMODCACHE=<.../module-cache>`;
- `GOPATH=<.../gopath>`;
- `TMPDIR=<.../tmp>`;
- `GOENV=off` whenever Go isolation is enabled.

Standard Go behavior should be preserved while generated outputs avoid host-global cache roots. Repository-local outputs are allowed only when the workflow explicitly requests repository-local output.

## 11. Diagnostics and Readback

Agent executions gain bounded mapping diagnostics.

Rust storage owner:

- `agent_executions.actual_toolchain_mapping_diagnostics_json`.

Northbound surfaces expose synthesized non-null diagnostics documents through:

- GraphQL `actualToolchainMappingDiagnostics`;
- MCP/report `actual_toolchain_mapping_diagnostics`;
- run report summaries.

Authoritative policy provenance:

- `runplan_snapshot` for compiled executions;
- `synthesized_legacy` for pre-column/legacy rows.

The live agent catalog is not authoritative for historical diagnostics.

Diagnostics states must explicitly distinguish:

- active;
- disabled by policy;
- policy absent;
- unsupported family;
- setup failed;
- queue timeout;
- legacy row unavailable;
- frozen snapshot contract incompatible.

Default northbound surfaces expose `TOOLCHAIN_HOME`-relative suffixes only. Absolute filesystem paths are debug-only and must be redacted out of GraphQL and MCP report payloads.

## 12. Cleanup and Housekeeping Readback

P066 adds two low-churn durable readback surfaces:

- `startupRecoverySummary.toolchainCache`;
- `toolchainCacheHousekeepingSummary`.

`startupRecoverySummary.toolchainCache` covers session-root recovery after daemon restart.

`toolchainCacheHousekeepingSummary` covers periodic run-root pruning and disk-pressure cleanup health.

These summaries must be compact. P066 must not write one row per file, stream chunk, or tool output.

## 13. Persistence Boundary

P066 complies with P075 once P075 lands.

Until then, P066 still follows these rules:

- no high-volume DB writes;
- no one-row-per-tool-output persistence;
- no long command logs in SQLite;
- compact runtime facts and artifact metadata pointers only;
- verbose evidence goes to files.

P066 does not introduce a new persistence model.

## 14. Side-Effect Boundary

P066 does not solve `settlement_incomplete`.

P066 must not:

- introduce `settlement_incomplete` statuses;
- add release reconciliation commands;
- block release retries;
- create side-effect tables;
- mutate release delivery receipts;
- change release stage settlement.

Release side-effect intent, settlement, reconciliation, and retry blocking belong to P078.

## 15. Scheduler Boundary

P066 must not make the scheduler language-aware.

Forbidden scheduler/workflow fields include:

- `language = swift`;
- `language = go`;
- `requires_xcode_cache`;
- `requires_go_cache`.

The scheduler may allocate provider capacity and runtime homes. Provider adapters decide how to map Xcode, Swift, Go, and future toolchains.

## 16. Operator-Visible Changes

- Provider-local cache directories land under `TOOLCHAIN_HOME/providers/{mapping_family}/{scope_key}/...`.
- Agent execution readback gains bounded `actualToolchainMappingDiagnostics`.
- Run reports distinguish Xcode queue wait from active build time.
- Cleanup proof becomes durable and named through startup recovery and housekeeping summaries.
- No new operator write action is introduced.

## 17. Metrics and Guardrails

| ID | Metric | Target | Breach action |
|---|---|---|---|
| M-001 | `mapping_setup_latency_p95_ms` | `<= 200` | Investigate storage/filesystem latency and hold rollout promotion. |
| M-002 | `supported_path_escape_count` | `0` | Immediate family rollback or rollout hold. |
| M-003 | `unsupported_mapping_warning_rate` | `0` for supported Xcode/standard Go flows | Investigate before promotion. |
| M-004 | `session_scoped_cleanup_success_rate` | `100%` | Hold promotion and inspect orphan cleanup. |
| M-005 | `run_scoped_root_prune_age_days` | `<= 7` | Investigate prune cadence and disk pressure. |
| M-006 | `xcode_lease_wait_p95_ms` | `<= 30000` during dogfood | Investigate same-run overlap, root quarantine, or workflow shape. |

Guardrails:

- no high-volume DB write pattern;
- no scheduler field, queue dimension, or capacity class;
- mapping detail remains bounded diagnostics payload;
- cleanup outcomes are aggregate telemetry, not per-execution history spam.

## 18. Rollout

### Phase 0: Scaffold

Land schema, compiler, adapter, host-executor, setup-failure, queue-diagnostics, and durable readback support with `toolchain_cache_policy.enabled=false` by default.

Phase 0 must prove:

- disabled execution emits `mapping_state=disabled_by_policy`;
- no mapped family roots are created while disabled;
- at least 10 legacy NULL rows and 10 post-migration rows synthesize sentinel behavior after restart;
- GraphQL, MCP, and report surfaces agree.

### Phase 1: Xcode Dogfood

Enable `xcode_scope=run` for Xcode-capable agent entries that already declare host execution.

Gate:

- at least 20 successful Xcode executions across at least 3 runs;
- zero path escapes;
- zero setup failures;
- zero queue timeout events;
- `mapping_setup_latency_p95_ms <= 200`;
- `xcode_lease_wait_p95_ms <= 30000`.

### Phase 2: Go Dogfood

Enable `go_scope=session` for Go-capable agent entries.

Gate:

- at least 20 successful Go executions across at least 5 sessions;
- zero host-global cache dependency;
- zero setup failures;
- zero path escapes;
- unsupported mapping warning rate is zero for standard Go flows.

### Phase 3: Default Catalog Backfill

Backfill explicit catalog defaults for supported agent entries once dogfood is stable.

Gate:

- seven consecutive daily sweeps show healthy cleanup;
- `session_scoped_cleanup_success_rate=100%`;
- `run_scoped_root_prune_age_days <= 7`;
- no unexpected quarantine creation.

## 19. Rollback Semantics

Catalog rollback affects future compilations only.

Already compiled, queued, resumed, and historical runs keep frozen run-plan truth. If immediate disablement is required for already compiled work, the operator must cancel and restart the affected run.

This tradeoff preserves historical readback coherence and avoids live catalog edits silently rewriting execution semantics.

If dogfood proves cancel-and-restart too slow for incident response, a dedicated execution-time kill-switch proposal may be considered later. It is not part of P066.

## 20. Acceptance Criteria

P066 is complete when:

1. The catalog accepts `agents[].toolchain_cache_policy` with exact versioned keys and rejects unknown keys/enum values.
2. Frozen catalog and run-plan snapshots fail closed when P066 policy appears without supported top-level format versions.
3. Rust and Swift preserve `toolchain_cache_policy` with the same object shape.
4. Swift operator-facing consumers decode through `ToolchainMappingReadAdapter`.
5. Agent executions persist bounded `actual_toolchain_mapping_diagnostics_json`.
6. GraphQL, MCP, and reports expose synthesized non-null diagnostics documents.
7. Diagnostics provenance is `runplan_snapshot` or `synthesized_legacy`, never live catalog.
8. Xcode-capable executions receive writable roots under `TOOLCHAIN_HOME/providers/xcode/{run_id}/xcode` by default.
9. Host-executed `xcodebuild` uses rewritten `-derivedDataPath`, `-clonedSourcePackagesDirPath`, and mapped `TMPDIR`.
10. Run-scoped Xcode queue wait has a bounded deadline and surfaces `xcode_run_scope_queue_timeout`.
11. Crash restart proves run-scoped Xcode root reuse is safe or quarantines before reuse.
12. Standard Go flows use mapped `GOCACHE`, `GOMODCACHE`, `GOPATH`, `TMPDIR`, and `GOENV=off`.
13. Mapping setup fails closed before toolchain work when required root creation or validation fails.
14. `startupRecoverySummary.toolchainCache` and `toolchainCacheHousekeepingSummary` exist before cleanup telemetry is used as a rollout gate.
15. Legacy NULL rows, post-migration rows, GraphQL, MCP, and reports synthesize promised sentinel behavior after restart.
16. No release-settlement logic, high-volume SQLite writes, or workflow-state semantic changes are introduced.
17. `./scripts/test-gate.sh proposal-066` exists and passes.

## 21. Test Plan

Add or keep:

```bash
./scripts/test-gate.sh proposal-066
```

The gate covers:

- catalog schema validation and unknown-key rejection;
- frozen snapshot compatibility rejection;
- Rust/Swift snapshot shape parity;
- Swift `ToolchainMappingReadAdapter` decoding and sentinel synthesis;
- diagnostics serialization and redaction;
- Xcode directory creation and command argument rewriting;
- Xcode run-scope serialization, queue timeout, and crash quarantine;
- Go environment mapping and `GOENV=off`;
- read-only runtime-home behavior;
- legacy-row and post-migration diagnostics synthesis;
- regression proving existing generic `TOOLCHAIN_HOME` behavior remains compatible.

Fake provider invocations are sufficient for readiness. Real Xcode, simulator, or Go network/module fetches are not required for proposal readiness.

## 22. Resolved Questions

### Q-001: Is a dedicated execution-time kill switch required?

Default answer: no.

Catalog rollback plus explicit cancel-and-restart is the P066 behavior. A separate kill-switch proposal is warranted only if dogfood shows that cancel-and-restart is operationally too slow for incidents.

### Q-002: Do non-Xcode Swift flows need first-class cache knobs?

Default answer: no for P066.

Xcode and standard Go flows are first-class. Non-Xcode Swift remains best-effort with explicit unsupported-mapping diagnostics until actual catalog usage proves a stable need.

### Q-003: Does housekeeping need broader `TOOLCHAIN_HOME` quota/trend data?

Default answer: not in P066.

P066 requires prune-age and disk-pressure cleanup summaries. Broader quota or trend analytics can be added later through housekeeping/P075 if the initial summaries are insufficient.

## 23. Final Recommendation

Approve P066 for implementation with the R4 contract freeze intact:

- top-level frozen-snapshot compatibility gating;
- `ToolchainMappingReadAdapter` as the Apple read owner;
- `runplan_snapshot`-only authoritative provenance;
- explicit rollback blast radius for snapshot-backed runs;
- named cleanup telemetry surfaces;
- separate Xcode queue-wait and setup-failure semantics.

P066 should stay focused. Its job is to make toolchain caches safe, isolated, and observable.
