# Proposal 056: Control-Plane Engine Modularization and Typed Internal Contracts

| Field | Value |
|---|---|
| Date | 2026-04-18 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [../reference/rust-control-plane.md](../reference/rust-control-plane.md), [local daemon lifecycle contract](../reference/local-daemon-lifecycle-supervision-and-packaging.md), [044-idea-crud-completeness-and-lifecycle-mcp-tools.md](044-idea-crud-completeness-and-lifecycle-mcp-tools.md), [../reference/per-run-workspace-isolation.md](../reference/per-run-workspace-isolation.md) |
| Scope | Tighten the internal contracts that live inside the control-plane Rust workspace so that the boundaries the daemon already exposes externally are also type-safe on the inside. |
| Goal | Remove the four remaining untyped seams inside `control-plane/` (work-item payload, `engine` god-crate, ACP session bootstrap JSON, `graphql-server::schema` monolith) and pin two new client-facing contracts (daemon-status snapshot+subscribe, ACP adapter config) without changing any externally observable behavior. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-056|p056`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context

The control-plane Rust workspace has grown to ~45 000 LOC across 9 crates. The architectural review in the 2026-04-18 lens (Russian text in-session) rated the overall shape **healthy**: acyclic crate DAG, domain without I/O, explicit state machines, fail-closed migration policy, 194 library tests green, zero `TODO` markers. The local daemon lifecycle contract is implemented and documented in `docs/reference/local-daemon-lifecycle-supervision-and-packaging.md`.

The review also called out five specific classes of remaining technical debt that are not covered by any open proposal:

1. The `engine` crate is 22 k LOC — roughly 45 % of the workspace — and holds at least eight independent responsibilities in a single flat module tree.
2. `engine::executor::process_item` destructures work-item payloads through 27 separate `payload["field"].as_str().unwrap_or(...)` sites. A typo turns into a runtime error during live agent execution rather than a compile-time error.
3. The ACP transport layer assembles `session/new` parameters via `serde_json::json!({ … })` with hand-written escape rules, and each adapter adds its own overrides through `Vec<Value>`. New adapters can drift silently.
4. `graphql-server/src/schema.rs` is a single 2.7 k-line file containing queries, mutations, subscriptions, GraphQL type wrappers, and nearly 40 unit tests.
5. The `daemonStatus` query and `daemonStatusChanged` subscription need a documented "snapshot + subscribe" client contract in the stable daemon lifecycle reference. Without that contract, a client that subscribes after startup and relies on pushed frames alone sees stale state.

None of these issues block shipping the product. All of them slow down every subsequent proposal that has to cut across one of these seams.

P056 addresses all five in one implementation track because they are coupled: typed work-item payloads require extracting `engine::executor` into a smaller crate; a cleaner schema file makes it easier to add the snapshot-plus-subscribe fixture; a shared ACP adapter config type makes per-adapter drift detectable in the same place where work-item payloads are typed.

## 2. Goals

- **G-1.** Every work-item kind has a typed payload struct; `process_item` branches match an enum and never read string keys from a `serde_json::Value`.
- **G-2.** The `engine` crate splits into a core (orchestrator + executor dispatch + work queue + event bus + cancellation + recovery + contracts + preflight + domain-engine + lifecycle reporter) and three feature crates (`engine-release`, `engine-steward`, `engine-session`). Consumers update their `Cargo.toml` but the public re-exports remain stable.
- **G-3.** ACP session-new parameters are built from a typed `AcpSessionNewParams` struct. Each adapter provides its overrides through a single `AcpAdapterConfig` struct with `Default` impls, not free-form JSON values.
- **G-4.** `graphql-server/src/schema.rs` splits into `queries.rs`, `mutations.rs`, `subscriptions.rs`, `types.rs`, and `root.rs` without losing any test; the existing `#[cfg(test)] mod tests` migrates to a `#[path = "tests.rs"] mod tests` submodule.
- **G-5.** The `daemonStatus` + `daemonStatusChanged` pair ships with an explicit "connect → snapshot → subscribe → resubscribe on lag" client contract captured in `docs/reference/control-plane-mcp.md` and enforced by a parity test that proves the snapshot shape equals the push shape.

## 3. Scope

P056 covers:

- Rust workspace refactor of `engine` into four crates
- Typed `WorkItemPayload` enum with `#[serde(tag = "kind")]` discriminant
- Typed `AcpSessionNewParams` + `AcpAdapterConfig` structs
- Split of `graphql-server::schema` into multiple files
- Documentation + parity test for `daemonStatus` snapshot/subscribe
- Canonical `proposal-056|p056` gate covering the four boundaries above

P056 does not cover:

- Any externally observable behavior change on GraphQL, MCP, ACP, or HTTP surfaces
- Any change to the SQLite migration sequence
- Any change to the workflow YAML schema or agent catalog format
- Daemon-lifecycle contracts (owned by `docs/reference/local-daemon-lifecycle-supervision-and-packaging.md`)
- Per-run workspace isolation (owned by P050)
- Swift app changes (must remain buying the same JSON shapes)

## 4. Problem Statement

### 4.1 The engine god-crate

`control-plane/crates/engine/src/` currently contains:

| Responsibility | LOC | Notes |
|---|---:|---|
| `orchestrator.rs` | 2.5 k | Run/stage/approval state machine |
| `executor.rs` | 2.6 k | Work-item dispatch + ACP spawn + artifact materialization |
| `release/` | ~1 k | Release coordinator, git push, connect publish, receipt |
| `steward/` | ~2 k | Steward cohort analyzer + dossier builder + config |
| `session/` | ~1.5 k | Session policy + budget + fingerprint |
| `contracts.rs` | 419 | Declared-output / companion-output validation |
| `command_handler.rs` | 646 | Command → work-item translation |
| `lifecycle_reporter.rs` | 257 | daemon lifecycle |
| everything else | ~4 k | `recovery`, `preflight`, `worktree`, `evidence`, `mcp`, `domain_engine`, `event_bus`, `work_queue`, `command_journal_redact` |

This is eight features in one crate. Any edit to a single one recompiles ~22 k LOC — the dev inner loop on an Apple Silicon M1 is around 18 s in incremental rebuilds and ~50 s on a clean build. The crate also has ~60 transitive dependencies and every consumer (`daemon`, `graphql-server`, `mcp-server`) picks up all of them.

### 4.2 Untyped work-item payloads

`engine::executor::process_item` deserializes `work_items.payload_json` as `serde_json::Value` and then destructures it by string key. An abridged excerpt from the `InvokeAgent` branch:

```rust
let provider = payload["provider"].as_str().ok_or_else(|| anyhow!("..."))?;
let model    = payload["model"].as_str().map(String::from);
let effort   = payload["effort"].as_str().map(String::from);
let worktree_write_enabled = payload["worktree_write_enabled"].as_bool().unwrap_or(false);
let worktree_strategy      = payload["worktree_strategy"].as_str().map(String::from);
// ...26 more fields…
```

Every place that enqueues an `InvokeAgent` hand-builds the JSON with `serde_json::json!({ "provider": ..., "model": ..., ... })`. The producer and consumer share no compile-time schema. A recent change renamed `backend_profile_id` in the compiler without touching the executor, and the bug was found by a runtime `None` in the live agent session, not by a compiler error. Typed payloads remove this class of bug entirely.

### 4.3 ACP session-new + adapter drift

`acp::transport::build_session_new_params` builds the `session/new` wire message with `serde_json::json!({ … })`. Each adapter (`claude.rs`, `codex.rs`, `gemini.rs`, `auggie.rs`, `junie.rs`) adds its own defaults through `config_options: Vec<Value>` and an ad-hoc `extra: Option<Value>`. Adding a sixth adapter today involves copying an existing file and adjusting ~12 hand-written keys. A typed `AcpAdapterConfig` with `#[derive(Default)]` per adapter would make the configuration surface uniform and auto-completable.

### 4.4 schema.rs monolith

`graphql-server/src/schema.rs` is 2.7 k lines. It contains:

- 11 query resolvers + `QueryRoot` impl
- 5 mutation resolvers + `MutationRoot` impl + 6 payload union types
- 5 subscription resolvers + `SubscriptionRoot` impl
- 2 GraphQL wrapper types (`GqlDaemonStatus`, `GqlRuntimeEvent`)
- 38 `#[cfg(test)]` tests

Opening the file takes ~4 s in VS Code, and every edit to the test block recompiles the whole schema. Splitting is straightforward — `async_graphql` supports multiple impl blocks across files and `#[derive(MergedObject)]` for root types.

### 4.5 Snapshot-plus-subscribe contract

The local daemon lifecycle contract ships `daemonStatus: DaemonStatus!` and `daemonStatusChanged: DaemonStatus!`. A client that only subscribes misses the current state because `tokio::broadcast` only delivers frames sent after subscribe. The correct pattern is:

1. Call `daemonStatus` once.
2. Immediately subscribe to `daemonStatusChanged`.
3. On `broadcast::Lag`, re-call `daemonStatus` and resume subscribing.

Today this is implicit. We need a documented contract and a parity test proving the snapshot response shape equals a subscription frame's shape.

## 5. Proposed Design

### 5.1 Typed work-item payloads

New module `engine::work_item_payload`:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkItemPayload {
    InvokeAgent(InvokeAgentPayload),
    SettleStage(SettleStagePayload),
    AdvanceRun(AdvanceRunPayload),
    RebuildProjection(RebuildProjectionPayload),
    StartupRepair(StartupRepairPayload),
    TriggerNextStage(TriggerNextStagePayload),
    StewardAnalysis(StewardAnalysisPayload),
}

#[derive(Serialize, Deserialize)]
pub struct InvokeAgentPayload {
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub backend_profile_id: Option<String>,
    pub permission_profile: Option<String>,
    pub prompt: String,
    pub task_name: String,
    pub task_inputs: Vec<String>,
    pub task_outputs: Vec<String>,
    pub worktree_write_enabled: bool,
    pub worktree_strategy: Option<String>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub skill_ref: Option<String>,
    pub skill_role: Option<String>,
    pub skill_snapshot_hash: Option<String>,
    pub requested_mcp_server_ids: Vec<String>,
    pub output_contract: Option<String>,
    pub max_turns: Option<i64>,
    pub temperature: Option<f64>,
    pub declared_outputs: Vec<DeclaredOutput>,
    // …and so on; one field per producer site today
}
```

Migration plan:

1. Land the struct + `#[serde(tag)]` variant without changing `work_items.payload_json` in SQLite.
2. Every producer (`orchestrator::enqueue_*`, `command_handler::handle`) calls `serde_json::to_value(&payload)?` — the enum discriminant is already wire-compatible with today's `json!({ "kind": "invoke_agent", ... })` shape because the existing producers already write `kind`.
3. `executor::process_item` switches from `payload["kind"]` match-on-string to `serde_json::from_value::<WorkItemPayload>(payload)?` with `match` arms. The hand-decoded path is deleted in the same commit.
4. Add a migration-fidelity test: load every `WorkItemKind` sample JSON used today, assert round-trip through `WorkItemPayload`.

### 5.2 Engine split

New crates:

```
engine-release  (was control-plane/crates/engine/src/release/)
engine-steward  (was control-plane/crates/engine/src/steward/)
engine-session  (was control-plane/crates/engine/src/session/)
engine          (everything else — orchestrator, executor, work_queue, lifecycle, recovery,
                 contracts, preflight, worktree, evidence, mcp, domain_engine, event_bus,
                 command_handler, cancellation, command_journal_redact)
```

Dependency direction:

```
engine (core) → engine-session → engine-steward → engine-release
```

i.e. each sub-crate depends on everything to its left. `daemon`, `graphql-server`, and `mcp-server` depend on `engine` (core) plus whichever feature crates they actually need. Dead imports are flagged by `cargo check` as part of the move.

The existing public API (`engine::command_handler::CommandHandler`, `engine::executor::BackgroundExecutor`, etc.) stays at the same path via `pub use` re-exports so no downstream `use` site changes in this refactor.

### 5.3 Typed ACP adapter config

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpAdapterConfig {
    pub binary_path_env: &'static str,
    pub default_model: Option<&'static str>,
    pub default_mode: AcpMode,         // bypass_permissions | ask | deny
    pub capability_class: &'static str,
    pub extra_extensions: BTreeMap<String, serde_json::Value>,
}

impl AcpAdapterConfig {
    pub fn claude() -> Self { … }
    pub fn codex() -> Self { … }
    pub fn gemini_cli() -> Self { … }
    pub fn auggie() -> Self { … }
    pub fn junie() -> Self { … }
}
```

`build_session_new_params` constructs a typed `AcpSessionNewParams` struct and serializes via `serde_json::to_value`. The hand-written `json!({ "cwd": effective_cwd, … })` path is deleted.

### 5.4 schema.rs split

```
control-plane/crates/graphql-server/src/schema/
├── mod.rs                  // type alias + build_schema + MergedObject assemblers
├── queries.rs              // QueryRoot + all #[Object] query methods
├── mutations.rs            // MutationRoot + all #[Object] mutation methods
├── subscriptions.rs        // SubscriptionRoot + all #[Subscription] methods
├── types.rs                // GqlDaemonStatus, GqlRuntimeEvent, payload unions
└── tests/                  // one test-mod per resolver set
    ├── queries_tests.rs
    ├── mutations_tests.rs
    ├── subscriptions_tests.rs
    └── contract_tests.rs   // P043 cross-surface parity + daemonStatus parity
```

`async_graphql::Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)` accepts the existing roots unchanged; nothing in the external GraphQL SDL moves.

### 5.5 snapshot-plus-subscribe contract

Add to `docs/reference/local-daemon-lifecycle-supervision-and-packaging.md` if the closeout reference does not already contain it:

> **Daemon status client contract.** A client that wants live daemon-lifecycle state must:
>
> 1. Open a GraphQL connection and issue `query { daemonStatus { ... } }`.
> 2. Subscribe to `daemonStatusChanged` on the same connection.
> 3. Treat `broadcast::Lag` as an instruction to re-issue `daemonStatus` and restart step 2.
>
> The snapshot response and every subscription frame carry identical JSON shapes; a client that only subscribes will observe the last broadcast state rather than the current state.

Parity test (Layer A, new): `test_daemon_status_snapshot_and_subscription_emit_identical_shape`. The test queries the snapshot, forces a transition, reads the subscription frame, and asserts the two JSON values match field-by-field (ignoring `last_state_change_at`).

## 6. Acceptance Criteria

- **AC-1.** `cargo test --workspace` passes with zero new failures.
- **AC-2.** No public API on `domain`, `db`, `acp`, `auth`, `graphql-server`, or `mcp-server` changes shape. Consumers verified by `cargo check -p daemon`.
- **AC-3.** `WorkItemPayload` enum has a variant for every `WorkItemKind` today (seven variants). A round-trip test loads a captured JSON for each variant and asserts zero loss.
- **AC-4.** `engine::executor::process_item` contains zero `payload["..."].as_` string-key reads after the migration.
- **AC-5.** `engine-release`, `engine-steward`, `engine-session` exist as separate crates; `cargo tree -p engine` shows the dependency direction `core ← session ← steward ← release`.
- **AC-6.** `acp::transport::build_session_new_params` takes a typed `AcpSessionNewParams` and serializes; every adapter in `acp::adapters/*.rs` constructs an `AcpAdapterConfig` via its `::default()` impl + targeted field overrides.
- **AC-7.** `graphql-server/src/schema.rs` is deleted; `graphql-server/src/schema/mod.rs` exists with `pub use schema::{QueryRoot, MutationRoot, SubscriptionRoot, build_schema}`.
- **AC-8.** `docs/reference/local-daemon-lifecycle-supervision-and-packaging.md` contains the snapshot+subscribe contract paragraph above.
- **AC-9.** The parity test `test_daemon_status_snapshot_and_subscription_emit_identical_shape` lives in `graphql-server/src/schema/tests/contract_tests.rs` and is registered in `PROPOSAL_056_TESTS`.
- **AC-10.** `./scripts/test-gate.sh proposal-056` runs the inventory below + `cargo test --workspace` + returns zero on a clean tree.

## 7. Risks

### 7.1 Crate-move churn

Moving ~6 k LOC across three new crates touches most `use engine::…` lines in downstream code. **Mitigation:** keep `engine` as the re-export root (`pub use engine_release::*;` etc.) so downstream `use engine::release::coordinator::…` keeps resolving. The refactor PR becomes mechanical + small per file.

### 7.2 Wire compatibility of typed payloads

If a typed payload drops a field the producer still emits, `serde_json::from_value` fails loudly. **Mitigation:** add `#[serde(default)]` for every `Option<T>` and `Vec<T>` during migration; once in-flight work items have drained (in-memory test), remove the defaults.

### 7.3 async-graphql split edge cases

`async_graphql::Schema` requires a single root type per query/mutation/subscription. Splitting impl blocks across files is supported; splitting the root type itself is not trivial. **Mitigation:** keep one `QueryRoot`/`MutationRoot`/`SubscriptionRoot` struct in `schema/mod.rs`; put the `#[Object]` / `#[Subscription]` impl blocks in sibling files. `async-graphql` allows multiple `#[Object]` blocks on the same root with feature `macros-utils`, which we already enable transitively.

### 7.4 Subscription frame ordering under lag

The snapshot+subscribe contract still has a race: between `daemonStatus` returning and `subscribe()` being called, a transition could fire. **Mitigation:** document that `last_state_change_at` monotonic; clients that care about transition ordering compare that timestamp against the snapshot. The parity test captures this by issuing the snapshot and subscribe from the same WebSocket connection.

## 8. Deliverables

| Deliverable | Owner | Surface |
|---|---|---|
| `engine::work_item_payload` typed enum + migration | control-plane | `control-plane/crates/engine/src/work_item_payload.rs` (new) |
| Engine split into four crates | control-plane | `control-plane/crates/engine{,-session,-steward,-release}/` |
| Typed ACP adapter config | control-plane | `control-plane/crates/acp/src/{config.rs,transport.rs}` |
| schema.rs split into module | control-plane | `control-plane/crates/graphql-server/src/schema/` |
| Snapshot+subscribe reference doc | control-plane docs | `docs/reference/local-daemon-lifecycle-supervision-and-packaging.md` |
| `proposal-056|p056` gate | control-plane | `scripts/test-gate.sh` |

## 9. Out of scope (follow-ups)

- Observability (logs rotation, redaction, `request_id` middleware) — owned by the local daemon lifecycle reference.
- ACP Xcode MCP bridge pooling — owned by P051.
- Swift client lifecycle UI — owned by the local daemon lifecycle reference for current daemon status behavior; future UI expansion needs its own owner.
- Deterministic startup latency — owned by [bounded artifact discovery and settlement optimization](../reference/artifact-discovery-and-settlement-optimization.md).
