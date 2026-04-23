# Proposal 066: Provider Toolchain Cache Mapping

| Field | Value |
|---|---|
| Date | 2026-04-22 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [rust-control-plane.md#provider-runtime-homes-and-toolchain-caches](../reference/rust-control-plane.md#provider-runtime-homes-and-toolchain-caches), [051-shared-xcode-mcp-bridge-pool.md](051-shared-xcode-mcp-bridge-pool.md) |
| Scope | Add adapter-local cache/build directory mappings for Swift/Xcode and Go provider sessions under the generic `TOOLCHAIN_HOME` contract documented in the Rust control-plane reference. |
| Goal | Daemon-launched agents that build Swift/Xcode or Go projects get writable, isolated, observable toolchain caches without making the scheduler or proposal workflow language-specific. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-066|p066`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

The implemented control-plane daemon provides the generic daemon/provider contract:

- every provider session gets `CHAINWORKS_TOOLCHAIN_HOME` and `TOOLCHAIN_HOME`;
- toolchain caches must live outside isolated runtime homes that can become read-only under provider sandboxing;
- orchestration must not encode "Rust-heavy", "Swift-heavy", or "Go-heavy" as first-class scheduler concepts.

The current implementation proves the contract with Codex/Rust because that is the failure currently blocking local control-plane work. Swift/Xcode and Go need the same treatment without changing the scheduler's language-neutral capacity model.

This proposal is the follow-up that turns the generic contract into concrete adapter-local mappings for:

- Xcode and Swift build/cache paths;
- Go build and module caches;
- future provider adapters that need language/toolchain-specific directory shaping.

---

## 2. Problem Statement

### 2.1 Xcode and Swift caches can leak into shared or read-only locations

Daemon-launched agents may invoke `xcodebuild`, `swift build`, SwiftPM package resolution, or Xcode MCP-backed operations. Without explicit mapping, those tools can write to:

- default DerivedData;
- module caches;
- SDK/stat caches;
- SwiftPM package checkouts and caches;
- runtime homes copied from operator config.

That can cause permission failures, cross-run contention, slow startup, or stale outputs that are hard to attribute to one run.

### 2.2 Go caches need the same isolation

Go support is still proposal-level unless a real `go.mod` appears, but once Go agents exist they need `GOCACHE`, `GOMODCACHE`, and related paths to be isolated per session/run/provider rather than inherited from the operator shell.

### 2.3 Scheduler language detection is the wrong abstraction

The executor should not decide that a task is "Swift" or "Go" and then mutate scheduling behavior. The scheduler can allocate capacity and writable cache roots. Provider adapters own the mapping from generic toolchain root to concrete environment variables and command arguments.

---

## 3. Scope

P066 includes:

- adapter-local mapping from `TOOLCHAIN_HOME` to Xcode/Swift cache and build directories;
- adapter-local mapping from `TOOLCHAIN_HOME` to Go cache and module directories;
- directory creation, permission validation, and structured logging before provider invocation;
- command-argument shaping where tools require flags instead of environment variables;
- tests that prove generated outputs do not land under read-only runtime homes or shared repo-global build directories;
- readback/log evidence that identifies the provider, run, stage, session, mapping family, and root without exposing secrets.

P066 does not include:

- introducing language-specific scheduler dimensions;
- changing the generic `TOOLCHAIN_HOME` contract;
- changing agent catalog semantics or reviewer routing;
- Xcode MCP bridge pooling; that remains P051;
- MCP profile/config isolation; that remains adjacent runtime config work;
- UI use of MCP tools or new GraphQL write paths.

---

## 4. Design

### 4.1 Root layout

The Rust control-plane reference owns the generic root:

```text
${CHAINWORKS_TOOLCHAIN_HOME:-$TOOLCHAIN_HOME}/
```

P066 maps tool-specific paths under provider/session scoped subtrees:

```text
providers/
  xcode/
    <session-or-run-key>/
      DerivedData/
      ModuleCache.noindex/
      SDKStatCaches/
      SourcePackages/
      tmp/
  go/
    <session-or-run-key>/
      build-cache/
      module-cache/
      gopath/
      tmp/
```

The isolation key should be configurable, but the default should be provider session or run-stage execution identity. It must not collapse every active run into one shared build directory.

### 4.2 Swift/Xcode mapping

The Xcode/Swift adapter layer should create and expose:

- `CHAINWORKS_XCODE_DERIVED_DATA_DIR`;
- `CHAINWORKS_XCODE_MODULE_CACHE_DIR`;
- `CHAINWORKS_XCODE_SDK_STAT_CACHE_DIR`;
- `CHAINWORKS_XCODE_SOURCE_PACKAGES_DIR`;
- `TMPDIR` under the same mapped root when the provider process supports it.

For `xcodebuild`, the adapter should prefer explicit arguments when available:

- `-derivedDataPath <.../DerivedData>`;
- `-clonedSourcePackagesDirPath <.../SourcePackages>`.

For SwiftPM or lower-level Swift tools, the adapter should set supported cache/module environment variables and avoid inventing unsupported flags. If a tool does not support an explicit cache path, the adapter records that limitation in runtime facts rather than pretending the mapping is complete.

### 4.3 Go mapping

The Go adapter layer should create and expose:

- `GOCACHE=<.../build-cache>`;
- `GOMODCACHE=<.../module-cache>`;
- `GOPATH=<.../gopath>`;
- `TMPDIR=<.../tmp>`;
- optional `GOENV=off` when provider isolation requires avoiding operator-global Go env files.

The adapter should preserve ordinary Go command behavior while keeping generated outputs out of repo-global paths unless the workflow explicitly requests a repository-local output.

### 4.4 Runtime facts and diagnostics

Provider launch should log and optionally persist runtime facts for:

- mapping family: `xcode`, `swift`, `go`;
- root directory;
- created directories;
- directory creation failures;
- unsupported mapping features;
- whether a path is session-, run-, or provider-scoped.

Runtime facts must not include access tokens, full command prompts, or personal config contents.

---

## 5. Acceptance Criteria

1. Xcode/Swift provider launches receive writable DerivedData, module-cache, SDK-stat-cache, SourcePackages, and tmp paths under `TOOLCHAIN_HOME`.
2. Xcode/Swift build invocations use explicit command arguments where supported and runtime facts where a tool cannot be fully redirected.
3. Go provider launches receive writable `GOCACHE`, `GOMODCACHE`, `GOPATH`, and tmp paths under `TOOLCHAIN_HOME`.
4. Concurrent runs do not share one repo-global Xcode DerivedData, SwiftPM package directory, Go build cache, or Go module cache by default.
5. A read-only provider runtime home does not break Swift/Xcode or Go cache writes.
6. The executor and scheduler do not gain language-specific capacity concepts; all language-specific behavior remains in provider adapters.
7. Logs/runtime facts expose the mapping decision and failures without leaking secrets.
8. Existing Codex/Rust behavior remains compatible with the generic `TOOLCHAIN_HOME` contract.

---

## 6. Implementation Outline

1. Add a small provider-toolchain mapping helper that takes provider, run, stage, session, and `TOOLCHAIN_HOME`.
2. Implement Xcode/Swift directory mapping and command-argument shaping in the Xcode/Swift provider boundary.
3. Implement Go directory mapping in the Go provider boundary when Go provider support becomes active.
4. Add runtime facts/logging for mapping decisions and unsupported tool features.
5. Add tests that simulate read-only runtime homes and assert cache writes go under `TOOLCHAIN_HOME`.
6. Add `proposal-066|p066` to the canonical test gate wrapper.

---

## 7. Test Plan

Add `./scripts/test-gate.sh proposal-066`.

The gate should include:

- unit tests for root layout and isolation-key derivation;
- Xcode/Swift adapter tests that inspect generated environment and command arguments;
- Go adapter tests that inspect `GOCACHE`, `GOMODCACHE`, `GOPATH`, and `TMPDIR`;
- read-only runtime-home tests proving cache directories are created under `TOOLCHAIN_HOME`;
- concurrency tests proving two active runs do not share the same generated output directories by default;
- regression tests proving existing Codex/Rust mappings still work.

The gate should use fake provider invocations by default. Real `xcodebuild`, simulator, or Go network/module fetches are not required for proposal readiness.

---

## 8. Risks and Tradeoffs

**Risk: Tool flags differ across Xcode/Swift versions.**
The adapter should prefer stable flags and record unsupported mappings instead of failing unrelated tasks.

**Risk: Over-isolation can reduce cache reuse.**
Session-scoped caches are safer but slower. P066 should allow configurable run/provider scoping after the default safe behavior is proven.

**Risk: Go support arrives later.**
The Go mapping can be implemented behind a provider-availability boundary. The contract still prevents future Go work from inheriting unsafe operator-global caches.

---

## 9. Open Questions

1. Should Xcode/Swift cache isolation default to session scope or run scope for better incremental build reuse?
2. Should Go module cache sharing be allowed per workspace when `go.sum` is stable, or remain isolated until dogfood data exists?
3. Which runtime surface should expose unsupported mapping facts: existing provider runtime facts, scheduler health, or a dedicated launch diagnostics projection?

---

## 10. Non-Goals Reaffirmed

P066 does not make Chainworks a language-aware scheduler. It keeps the orchestration contract language-neutral and puts concrete cache/build path knowledge at the provider adapter boundary.
