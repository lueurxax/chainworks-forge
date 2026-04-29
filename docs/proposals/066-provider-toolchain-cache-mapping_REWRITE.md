# Proposal 066 Rewrite: Provider Toolchain Cache Mapping with Persistence and Side-Effect Boundaries

| Field | Value |
|---|---|
| Date | 2026-04-29 |
| Status | Rewrite / Scope Clarification |
| Author | Engineer (single-engineer project) |
| Rewrites | `066-provider-toolchain-cache-mapping.md` |
| Depends on | Rust control-plane provider runtime homes and toolchain caches, Xcode MCP bridge pool |
| Related | P075 Local Persistence Write Budget, P078 Durable Side-Effect Ledger |
| Scope | Add adapter-local cache/build directory mappings for Swift/Xcode and Go provider sessions under the generic `TOOLCHAIN_HOME` contract. Explicitly exclude release side-effect settlement and SQLite write-budget work. |
| Goal | Give daemon-launched provider sessions writable, isolated, observable toolchain caches without changing scheduler semantics, workflow semantics, release settlement semantics, or the local persistence model. |

---

## 1. Why this rewrite exists

P066 began as a provider toolchain cache mapping proposal.

During implementation/review, a deeper issue surfaced:

> external release side effects can complete while the control plane fails before durable settlement.

That issue is important, but it is not P066.

P066 must remain focused on cache/build path isolation.

The side-effect settlement problem belongs to P078.
The SQLite write-pressure problem belongs to P075.

This rewrite clarifies the boundary so P066 does not become a dumping ground for release recovery, persistence architecture, or workflow semantics.

---

## 2. Core scope

P066 owns:

- Xcode/Swift cache mapping,
- Go cache mapping,
- provider-local writable toolchain roots,
- command-argument shaping where tools support explicit cache/build paths,
- environment variable shaping where tools use env vars,
- directory creation and permission validation,
- runtime facts/logging for mapping decisions,
- tests proving generated outputs do not land in shared or read-only runtime homes.

P066 does not own:

- release side-effect settlement,
- durable effect lifecycle,
- retry blocking,
- reconciliation,
- SQLite write budgeting,
- evidence spooling,
- workflow transition semantics,
- scheduler language awareness,
- GraphQL/MCP control boundaries,
- Xcode MCP bridge pooling.

---

## 3. Context

The daemon/provider contract provides generic roots:

- `CHAINWORKS_TOOLCHAIN_HOME`
- `TOOLCHAIN_HOME`

Provider runtime homes may be copied from operator configuration and may become read-only under provider sandboxing.

Build tools must therefore not rely on:

- global DerivedData,
- operator-global SwiftPM caches,
- operator-global Go caches,
- repo-global generated build outputs,
- provider runtime homes as writable cache roots.

P066 maps generic toolchain roots into tool-specific writable paths.

---

## 4. Root layout

Suggested layout:

```text
{TOOLCHAIN_HOME}/
  providers/
    {provider_family}/
      {session_or_run_key}/
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

The isolation key should be configurable.

Default:

- session-scoped for safety,
- run-scoped only where incremental build reuse is explicitly enabled.

Never default to one shared repo-global build directory for all active runs.

---

## 5. Xcode and Swift mapping

The adapter should create and expose:

- `CHAINWORKS_XCODE_DERIVED_DATA_DIR`
- `CHAINWORKS_XCODE_MODULE_CACHE_DIR`
- `CHAINWORKS_XCODE_SDK_STAT_CACHE_DIR`
- `CHAINWORKS_XCODE_SOURCE_PACKAGES_DIR`
- `TMPDIR`

For `xcodebuild`, prefer explicit arguments where supported:

```text
-derivedDataPath <.../DerivedData>
-clonedSourcePackagesDirPath <.../SourcePackages>
```

For SwiftPM / lower-level Swift tools, use supported environment variables and avoid unsupported flags.

If a tool cannot be fully redirected, record an unsupported-mapping fact rather than pretending the mapping is complete.

---

## 6. Go mapping

The adapter should create and expose:

- `GOCACHE=<.../build-cache>`
- `GOMODCACHE=<.../module-cache>`
- `GOPATH=<.../gopath>`
- `TMPDIR=<.../tmp>`

Optional:

- `GOENV=off` when provider isolation requires avoiding operator-global Go env files.

The adapter must preserve ordinary Go command behavior while keeping generated outputs out of repo-global paths unless the workflow explicitly requests repository-local output.

---

## 7. Runtime facts

P066 may record compact runtime facts:

- provider family,
- mapping family (`xcode`, `swift`, `go`),
- root path,
- isolation key,
- created directories,
- validation failures,
- unsupported mapping features,
- whether mapping was session-, run-, or provider-scoped.

P066 must not record high-volume tool evidence in SQLite.

If a command emits stdout/stderr or long logs, that evidence should be spooled according to P075.

---

## 8. Persistence boundary

P066 must comply with P075 once P075 lands.

Until P075 lands, P066 should still follow these rules:

- do not create high-volume DB writes;
- do not write one DB row per tool output;
- do not persist long command logs into SQLite;
- persist only compact runtime facts and artifact metadata pointers;
- write verbose evidence to files.

P066 should not introduce a new persistence model.

---

## 9. Side-effect boundary

P066 does not solve `settlement_incomplete`.

If implementation discovers release side-effect risk, the fix belongs to P078.

P066 must not:

- introduce `settlement_incomplete` statuses,
- add release reconciliation commands,
- block release retries,
- create side-effect tables,
- mutate release delivery receipts,
- change release stage settlement.

It may reference P078 as the systemic owner.

---

## 10. Scheduler boundary

P066 must not make the scheduler language-aware.

The scheduler may allocate provider capacity and runtime homes.

Provider adapters decide how to map:

- Xcode/Swift tools,
- Go tools,
- future toolchains.

No scheduler fields like:

- `language = swift`,
- `language = go`,
- `requires_xcode_cache`,
- `requires_go_cache`

should be introduced by P066.

---

## 11. MCP / GraphQL boundary

P066 does not add UI actions.

P066 does not add GraphQL mutations.

P066 does not add MCP tools unless a narrow diagnostics/readback tool is explicitly required later.

Any future toolchain diagnostics should be read-only and should not become an operator control path.

---

## 12. Tests

Add or keep gate:

```text
./scripts/test-gate.sh proposal-066
```

Required tests:

1. Xcode/Swift mapping creates expected directories under `TOOLCHAIN_HOME`.
2. `xcodebuild` command shaping uses explicit derived data/source package paths when supported.
3. SwiftPM unsupported mapping features are recorded as facts, not faked.
4. Go mapping sets `GOCACHE`, `GOMODCACHE`, `GOPATH`, and `TMPDIR`.
5. Read-only runtime home does not break cache writes.
6. Concurrent runs do not share one default repo-global build/cache directory.
7. Existing Codex/Rust behavior remains compatible with generic `TOOLCHAIN_HOME`.
8. Runtime fact persistence remains compact and does not store verbose logs.

---

## 13. Acceptance criteria

P066 is complete when:

1. Swift/Xcode provider sessions receive isolated writable toolchain directories.
2. Go provider sessions receive isolated writable toolchain directories.
3. generated build/cache outputs avoid read-only runtime homes and unsafe shared global roots.
4. adapter logs/runtime facts show mapping decisions without leaking secrets.
5. no scheduler language-specific capacity concepts are introduced.
6. no release side-effect settlement logic is added to P066.
7. no high-volume SQLite writes are introduced by P066.
8. P066 gate passes.

---

## 14. Final recommendation

P066 should stay boring.

Its job is not to solve release settlement.
Its job is not to redesign persistence.
Its job is to make toolchain caches safe, isolated, and observable.

The deeper problems discovered while reviewing P066 are real, but they belong to:

- P075 for write discipline and evidence spooling,
- P078 for durable side-effect settlement and reconciliation.
