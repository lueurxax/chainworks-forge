# Proposal 070: Control-Plane Architecture Consolidation and Typed Boundary Refactor

| Field | Value |
|---|---|
| Date | 2026-04-24 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [044-idea-crud-completeness-and-lifecycle-mcp-tools.md](044-idea-crud-completeness-and-lifecycle-mcp-tools.md), [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [056-control-plane-engine-modularization-and-typed-contracts.md](056-control-plane-engine-modularization-and-typed-contracts.md), [063-mcp-tool-response-shaping-and-field-selection.md](063-mcp-tool-response-shaping-and-field-selection.md), [068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md](068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md), [rust-control-plane.md](../reference/rust-control-plane.md), [execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md), [mcp-northbound-control-plane-server.md](../reference/mcp-northbound-control-plane-server.md), [per-run-workspace-isolation.md](../reference/per-run-workspace-isolation.md) |
| Scope | Consolidate the Rust control-plane architecture after several proposal-driven feature increments by restoring clear crate boundaries, typing internal contracts, extracting shared read/surface contracts, and moving workflow-specific policy out of generic engine paths. |
| Goal | Make the control plane easier to change safely by turning current implicit JSON, duplicated surface logic, and monolithic orchestration paths into explicit typed modules with enforceable dependency and parity gates, while preserving externally observable behavior. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-070|p070`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

**Relationship to P056:** P056 remains useful as the first narrower statement of typed work-item payloads, engine modularization, ACP session params, and GraphQL schema splitting. P070 supersedes the overlapping implementation plan because the control-plane has grown beyond P056's assumptions and now also needs MCP/GraphQL boundary repair, shared read models, workflow-extension extraction, capability registry consolidation, and test-suite reorganization. If P056 is implemented first, P070 must treat it as a prerequisite and delete duplicate work from this proposal before implementation.

---

## 0. Pending Proposal Alignment

P070 is an architecture-consolidation proposal, not a replacement for every pending product proposal. Its job is to provide the seams those proposals should land on, and to migrate any proposal that lands first out of legacy seams afterward.

### 0.1 Alignment rule

Pending proposals should be implemented in one of two modes:

- **Aligned-first mode:** a small P070 phase lands before the feature proposal, giving the feature a typed boundary to use immediately.
- **Migrate-after mode:** the feature proposal lands on the current code path, then P070 migrates that behavior into the new boundary without changing external behavior.

The implementation queue should prefer aligned-first mode for shared surfaces, command payloads, and workflow policy because those are the places where proposal-driven drift has accumulated.

### 0.2 Proposal lanes

| Lane | Pending proposals and stable references | P070 architecture owner | Alignment requirement |
|---|---|---|---|
| MCP/API surface and agent boundary | [044](044-idea-crud-completeness-and-lifecycle-mcp-tools.md), [045](045-run-recovery-and-granular-retry-mcp-tools.md), [049](049-context-strategy-management-mcp-tools.md), [063](063-mcp-tool-response-shaping-and-field-selection.md), [068](068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md) | `control-plane-surface`, typed MCP params/results, shared capability registry | New tools and resources must register once, parse typed params at the boundary, return typed/compact DTOs, and never use GraphQL as the agent fallback. |
| GraphQL UI and shared read models | [thin UI contract](../reference/query-projections-and-client-consumption-contract.md), [session observability GraphQL readback](../reference/rust-control-plane.md#graphql), [047](047-yaml-validation-and-definition-inspection-api.md), [069](069-bounded-discovery-diagnostics-operator-ui.md) | `control-plane-read-model`, GraphQL adapter, MCP adapter | UI fields that also matter to MCP should originate below both surfaces. GraphQL remains UI-facing; MCP remains agent-facing. |
| Runtime recovery, retry, and loop policy | [037](037-acp-execution-supervision-and-idle-watchdog.md), [045](045-run-recovery-and-granular-retry-mcp-tools.md), [052](052-orchestrator-loop-budget-source-of-truth.md), [062](062-implementation-approval-rejection-loopback.md), [065](065-operator-retry-instruction-contract.md), [071](071-explicit-workflow-transition-tie-break-syntax.md) | `engine-core`, `engine-runtime`, workflow extension contracts | Retry, watchdog, loop budget, approval loopback, operator retry instructions, and explicit transition selection must not add more hard-coded workflow policy to `orchestrator.rs`. |
| Provider/session/toolchain reliability | [034](034-clean-yaml-runtime-transport-normalization.md), [Xcode MCP bridge pool](../reference/xcode-mcp-bridge-pool.md), [055](055-control-plane-debug-launch-supervision-hardening.md), [provider toolchain cache mapping](../reference/acp-runtime-transport.md#toolchain-cache-mapping) | `acp`, `engine-session`, `engine-runtime`, daemon startup composition | Provider-specific runtime homes, bridge pools, watchdogs, and debug supervision should live behind provider/session/startup boundaries, not in workflow policy. |
| Worktree, release, evidence, and cleanup safety | [038](038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md), [039](039-blocked-run-fork-and-canonical-carry-forward.md), [059](059-release-evidence-gates-and-approval-payload-contract.md), [064](064-run-worktree-main-sync-and-cross-run-knowledge-transfer.md) | `engine-release`, worktree service, artifact/read model services, safety gates | Worktree and artifact lifecycle changes must preserve durable-work proof before cleanup and expose shared read DTOs for UI/MCP inspection. |
| Lead/steward routing and workflow-specific intelligence | [023](023-loop-improvement-analytics-and-iteration-progression.md), [048](048-steward-recommendation-lifecycle-and-experiment-tracking.md), [deterministic proposal-review routing](../reference/workflow-execution-engine.md#system-tasks-and-deterministic-proposal-review-routing), [067](067-lead-decomposed-implementation-slices-and-capability-minimal-agent-routing.md) | `engine-steward`, workflow extension contracts, capability-minimal routing registry | Lead/steward behavior should be workflow-extension or steward-owned. It should not become generic engine branching keyed by proposal workflow state names. |
| Long-term extraction direction | [1000](1000-go-temporal-control-plane-extraction.md) | typed domain/runtime/read contracts | P070 should make a future extraction easier by clarifying contracts, but P070 does not choose Go/Temporal or change the current Rust/SQLite operating model. |

### 0.3 Ordering guidance

P070 should not freeze the product queue, but it should front-load the architecture cuts that unblock many proposals at once:

1. **P070 Phase 0-1 before broad MCP expansion where possible.** The neutral surface crate and shared capability registry should land before major P044/P045/P049/P063/P068 tool additions. If those proposals land first, P070 must migrate their tool metadata and auth mapping into the shared registry.
2. **P070 Phase 2 before retry/workflow payload churn where possible.** Typed work-item payloads should land before P052/P062/P064/P065/P067 add more scheduler/orchestrator payload fields. If they land first, they must add compatibility fixtures that P070 migrates.
3. **P070 Phase 4 before duplicated UI/MCP read work where possible.** Thin UI follow-ons, session observability, P047, and P069 should consume shared read DTOs for any field also needed by MCP. UI-only presentation fields may remain GraphQL-specific.
4. **P070 Phase 6 before more Chainworks-specific workflow branching.** P052, deterministic proposal-review routing, P062, P064, P065, and P067 should use workflow extension contracts instead of adding more state-name checks to generic engine code. P071 should use compiled workflow metadata and generic transition selection services, not workflow-specific branches.
5. **Provider/session work can run in parallel with P070 if it respects boundaries.** P034/P037/P051/P055 and the implemented provider toolchain cache mapping may proceed while P070 is in progress, but provider-specific behavior must stay in ACP/session/daemon-startup services rather than leaking into workflow orchestration.

### 0.4 Proposal conflict policy

When a pending proposal conflicts with P070's target architecture:

- the product behavior in the older proposal remains valid unless P070 explicitly marks it obsolete;
- the implementation location should change to the P070 boundary;
- tests from the older proposal should become behavior tests under the P070-owned module or crate;
- reference docs should describe the final implemented architecture, not the temporary migration path.

---

## 1. Architecture Review Summary

An architecture pass over `control-plane/` on 2026-04-24 found that the original workspace shape is still recognizable and worth preserving. In the diagrams below, `A <- B` means `B` depends on `A`:

```text
domain <- auth
domain <- db
domain <- workflow
domain <- acp
domain, auth, db, workflow, acp <- engine
domain, auth, db, engine <- graphql-server
domain, auth, db, engine <- mcp-server
all runtime crates <- daemon
```

The main issue is not that the architecture is wrong. The issue is that repeated proposal-era feature increments have placed too much policy into the fastest-moving seams:

- `engine` now contains execution dispatch, workflow orchestration, ACP invocation, artifact settlement, recovery, release coordination, steward analysis, session policy, worktree management, and command handling in one broad module tree.
- `graphql-server` and `mcp-server` both shape operator-facing data, but there is no shared read model. Each surface enriches run/stage/artifact truth on its own.
- MCP, GraphQL, auth, and domain each carry pieces of the capability/tool registry. This creates drift risk as P068 makes MCP the primary agent surface.
- Work-item payloads, MCP tool params, ACP session params, and several durable diagnostic payloads remain free-form JSON inside the process instead of typed Rust values serialized at the storage/wire edge.
- Generic engine code now contains Chainworks proposal-workflow-specific states and prompts, such as implementation approval rejection context.

The result is an understandable but fragile architecture: most behavior works, but future proposals must cut through large files and implicit contracts.

### 1.1 Current size signals

The control-plane Rust source is now materially larger than the P056 baseline.

| Area | Current signal |
|---|---:|
| `control-plane/crates/**/src/**/*.rs` | 63,945 LOC |
| `control-plane/crates/**/tests/**/*.rs` | 27,973 LOC |
| `engine/src/executor.rs` | 6,337 LOC |
| `engine/src/orchestrator.rs` | 5,734 LOC |
| `graphql-server/src/schema.rs` | 4,092 LOC |
| `domain/src/discovery.rs` | 2,696 LOC |
| `mcp-server/src/server.rs` | 2,224 LOC |
| `acp/src/transport.rs` | 2,126 LOC |
| `engine/src/command_handler.rs` | 1,935 LOC |
| `db/src/repos/scheduler.rs` | 1,603 LOC |
| `db/src/repos/artifact_contracts.rs` | 1,447 LOC |
| `mcp-server/src/tools/reports.rs` | 1,269 LOC |
| `engine/src/session/policy.rs` | 1,221 LOC |

These numbers are not failures by themselves. They are evidence that the codebase has crossed the point where proposal-specific changes should continue landing in the same central files.

---

## 2. Findings

### 2.1 Layering drift: MCP depends on GraphQL internals

`mcp-server` currently depends on `graphql-server` so MCP HTTP can read the `graphql_server::request_id::RequestId` extension inserted by shared axum middleware. This contradicts `docs/reference/rust-control-plane.md`, which describes GraphQL and MCP as sibling server surfaces over engine/db.

This is a small dependency edge with a large architectural meaning:

- It makes GraphQL a shared infrastructure crate by accident.
- It weakens P068's boundary that GraphQL is UI-only while MCP is the agent/operator automation surface.
- It encourages future cross-surface imports instead of a small shared HTTP/surface contract module.

The correct direction is to extract request-id middleware, caller correlation helpers, capability IDs, and any shared surface DTOs into a neutral crate that both server surfaces can depend on.

### 2.2 Engine has become a multi-domain coordinator

`engine/src/executor.rs` and `engine/src/orchestrator.rs` now own too many responsibilities:

- work-item claim/complete/fail lifecycle;
- provider capacity and backpressure;
- ACP session bootstrap and live invocation;
- artifact discovery, validation, settlement, and recovery evidence;
- workflow transition advancement;
- approval handling and rejection prompts;
- implementation handoff and code-writer status;
- run worktree provisioning and cleanup;
- steward enqueueing;
- release and lifecycle reporting hooks;
- projection rebuild scheduling.

The largest operational risk is not file length alone. The risk is that one change to a workflow policy, artifact contract, or provider invocation path can accidentally alter queue semantics, recovery truth, or transition behavior because these concerns are interleaved in the same functions.

### 2.3 Work-item payloads are internal JSON contracts

`db::work_item::WorkItem` stores `payload_json: String`, and `engine::work_queue::WorkQueue` accepts `serde_json::Value`. Producers hand-build payloads with `serde_json::json!`, while consumers parse them with string-key access in executor and scheduler code.

Concrete examples:

- `engine::orchestrator::enqueue_invoke_agent` constructs a broad `InvokeAgent` JSON payload containing run/stage IDs, provider config, prompt, MCP server IDs, session policy, declared outputs, worktree strategy, and degraded-output policy.
- `engine::executor` later reparses the same payload and extracts fields by name.
- `db::repos::scheduler` parses `payload_json` to infer provider family, stage execution ID, and startup recovery flags for queue summaries.

This makes `payload_json` a hidden schema shared by `engine`, `db`, and tests. It should become a typed enum with versioned serde compatibility. JSON should remain the SQLite storage format, not the internal programming model.

### 2.4 MCP tool params/results are manually typed at runtime

MCP tool specs declare JSON schemas manually, but tool handlers accept `serde_json::Value` and read fields such as `params["run_id"].as_str()`. Results are shaped with `serde_json::json!` and often duplicate GraphQL enrichment logic.

This conflicts with the direction in P068 and P063:

- MCP is becoming the primary agent surface, so it needs strong contracts.
- Compact field selection and include behavior will drift if every tool hand-shapes its own JSON.
- Capability policy is harder to audit when tool names, capability IDs, input schemas, and handlers are defined in separate hand-maintained match statements.

### 2.5 GraphQL and MCP read models are duplicated

GraphQL enriches runs with artifact contract projections, workflow conflicts, implementation handoff status, runtime facts, and other derived fields. MCP tools/resources perform their own enrichment and serialization through separate code paths.

This creates two risks:

- UI and MCP can disagree about the same run state.
- Adding a new durable execution-truth field requires edits in db repos, GraphQL types/resolvers, MCP tools/resources, tests, and reference docs with no central read contract.

The right abstraction is not to make MCP import GraphQL or GraphQL import MCP. Both should consume shared read-model/query services that return typed DTOs, then adapt those DTOs to their transport-specific envelopes.

### 2.6 Generic engine contains workflow-specific proposal policy

`engine::orchestrator` contains hard-coded proposal-workflow states and operator-facing prompt text, including behavior around `state_5_proposal_refined` and `state_6_implementation_approval`.

This makes the generic workflow engine less generic:

- Adding a second workflow family risks more hard-coded state IDs.
- Refactoring proposal review/implementation loops requires editing central transition code.
- Tests for a specific Chainworks workflow become entangled with engine-wide transition tests.

Workflow-specific behavior should move behind a typed extension/strategy boundary owned by workflow compilation or a Chainworks workflow adapter layer.

### 2.7 DB repositories mix persistence, read models, and scheduling policy

The repository layer is no longer just persistence. Large modules such as `db/src/repos/scheduler.rs`, `artifact_contracts.rs`, and `work_items.rs` include query projection assembly, queue health derivation, status transitions, and payload parsing.

Some of this belongs in repositories, but the current split makes transaction ownership and behavioral ownership hard to see:

- scheduler queue summaries parse work-item payload JSON;
- artifact contract repos export read-model JSON used by both surfaces;
- command/use-case transaction boundaries are spread across engine and db repos.

P070 should not create a pure repository abstraction for its own sake. It should draw clearer use-case boundaries: persistence primitives stay in `db`, read models move to a shared query layer, and transactional command flows are owned by command/runtime services.

### 2.8 Capability and principal policy can drift across surfaces

Capability IDs are defined in `domain`, default principal policy is in `auth`, MCP maps tool names to capability IDs in `mcp-server`, and GraphQL maps mutations to capability IDs in `graphql-server`.

That split was acceptable when the surface area was small. With P068, capability mapping becomes product-critical. There should be one registry that connects:

- stable capability ID;
- MCP tool name;
- resource template ID where applicable;
- GraphQL mutation, if still exposed for UI;
- allowed principal classes;
- command-journal caller identity;
- tests proving every mutating surface has a policy.

### 2.9 Test organization mirrors proposal history, not behavior ownership

The control-plane tests contain large integration files and proposal-number-specific suites. Examples include:

- `engine/tests/integration.rs` at 8,361 LOC;
- `db/tests/integration.rs` at 3,172 LOC;
- `engine/tests/proposal_061_backpressure.rs` at 2,405 LOC;
- `acp/tests/integration.rs` at 2,001 LOC.

Proposal-number tests are useful while implementing a proposal. Over time they become hard to search, hard to compose, and hard to use as ownership documentation. Stable behavior suites should be organized around runtime domains such as work queue, scheduler, workflow transitions, artifact settlement, MCP surface, auth/capabilities, and recovery.

### 2.10 Reference docs and proposals now lag code reality

`docs/reference/rust-control-plane.md` still communicates the desired architecture, but the current workspace has drifted:

- actual dependencies include `mcp-server -> graphql-server`;
- `auth` is now an important workspace crate;
- P056's size assumptions are stale;
- GraphQL/MCP boundaries have changed under P068;
- execution truth is spread across domain, db, engine, GraphQL, and MCP code paths.

Docs do not need to describe every module, but they must describe the architecture that gates future refactors.

---

## 3. Goals

- **G-1. Restore crate layering.** `graphql-server` and `mcp-server` are sibling surfaces. Shared HTTP/request/capability code moves to neutral shared crates.
- **G-2. Type internal contracts.** Work items, MCP params/results, ACP session params, command outcomes, and shared read DTOs are typed Rust values internally.
- **G-3. Extract shared read models.** GraphQL and MCP consume the same query/read-model services for run, stage, artifact, scheduler, daemon, and recovery views.
- **G-4. Split engine by runtime domain.** Execution dispatch, workflow transition planning, agent invocation, artifact settlement, session policy, steward, release, and worktree policy become separately owned modules or crates with clear dependency direction.
- **G-5. Move workflow-specific policy out of generic engine code.** Chainworks proposal/review/implementation-loop behavior lives behind workflow extension contracts.
- **G-6. Consolidate capability registry.** Tool/resource/mutation capability mapping and principal policy have one source of truth.
- **G-7. Reorganize tests around behavior ownership.** Proposal-era tests either become stable behavior suites or remain as short compatibility/golden tests.
- **G-8. Add enforceable architecture gates.** Dependency rules, typed boundary tests, and parity checks prevent the same drift from returning.

---

## 4. Non-Goals

- No externally observable GraphQL, MCP, ACP, or SQLite migration behavior should change as part of mechanical extraction.
- No rewrite of the macOS app.
- No replacement of SQLite.
- No removal of proposal-era tests until equivalent stable behavior coverage exists.
- No cleanup of run-owned worktrees, `.chainworks` state, target directories, or databases.
- No forced "big bang" module move. P070 must be implementable in safe increments with behavior-preserving gates after each phase.

---

## 5. Proposed Target Shape

### 5.1 Crate and module boundaries

P070 should move toward this dependency shape:

```text
domain
  <- auth                         # principal table and capability filtering
  <- control-plane-surface        # request id, capability registry, surface DTO ids
  <- db                           # persistence primitives and migrations
  <- workflow                     # YAML compile/runtime plan contracts
  <- acp                          # ACP transport and adapter protocol

domain, auth, db, workflow, control-plane-surface
  <- control-plane-read-model     # typed query DTOs and projection assembly

domain, db, workflow, acp, control-plane-read-model
  <- engine-core                  # transition planning, command use cases, work queue contracts
  <- engine-runtime               # executor, agent invocation, artifact settlement, recovery
  <- engine-session               # session policy, budget, fingerprinting
  <- engine-steward               # steward analysis/dossier
  <- engine-release               # release coordinator and receipts
  <- engine-chainworks-workflow    # proposal/review/implementation-loop extensions

control-plane-surface, control-plane-read-model, engine-core
  <- graphql-server               # UI GraphQL adaptation only
  <- mcp-server                   # MCP/agent adaptation only

all runtime crates
  <- daemon                       # composition root
```

The exact crate names can change during implementation, but the dependency rule cannot: server surfaces do not depend on each other, workflow-specific policy does not live in generic engine transition code, and read-model DTOs are shared below both surfaces.

### 5.2 Neutral surface contract crate

Introduce a small shared crate, tentatively `control-plane-surface`, for:

- `RequestId` and request-id validation;
- request-id middleware helpers that do not depend on GraphQL;
- stable capability IDs and surface names;
- MCP tool/resource registry metadata;
- GraphQL mutation registry metadata for UI-owned mutations;
- typed caller context helpers if they do not belong in `domain::commands`;
- transport-neutral error codes used by GraphQL/MCP adapters.

This crate must not depend on `graphql-server`, `mcp-server`, `engine`, or `db`.

The first implementation milestone should remove the `mcp-server -> graphql-server` dependency.

### 5.3 Typed work-item payloads

Add a versioned `WorkItemPayload` enum:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
```

Compatibility requirements:

- The SQLite `work_items.payload_json` column remains JSON text.
- Existing pending/running rows can still be decoded.
- Producers serialize typed payloads at enqueue time.
- Consumers deserialize into typed payloads at the boundary and never access `payload["field"]` in runtime logic.
- Scheduler queue summaries get typed helper methods for provider family, stage execution ID, and startup recovery markers.

### 5.4 Typed MCP tools

Each MCP tool should define:

- params struct;
- result struct;
- error code enum where behavior is not generic JSON-RPC failure;
- input schema generated or derived from the params struct where practical;
- capability ID from the shared registry;
- compact default result and explicit include options for heavier fields.

Manual JSON schema is acceptable during migration, but it must be generated from or tested against the params struct. Tool handlers should parse once at the boundary and pass typed values into command/read services.

### 5.5 Shared read-model/query layer

Introduce a shared query layer, tentatively `control-plane-read-model`, for durable operator views:

- run summary and run detail;
- stage summary and stage detail;
- artifact/report listing and detail metadata;
- implementation handoff status;
- workflow conflict status;
- scheduler health and queue summaries;
- daemon lifecycle status;
- recovery/stale execution views;
- principal-visible capability/resource lists.

GraphQL maps these DTOs into GraphQL objects. MCP maps the same DTOs into tool results or resource payloads. Any field that exists on both surfaces must have one source of truth below the surfaces.

### 5.6 Engine runtime split

The engine should be split by behavior ownership before it is split by file size:

| Area | Responsibility |
|---|---|
| `engine-core` | Commands, transition planning, work-queue contracts, domain events, cancellation contracts. |
| `engine-runtime` | Work-item executor, agent invocation lifecycle, artifact settlement, recovery evidence, projection rebuild triggers. |
| `engine-session` | Provider session reuse policy, budgets, fingerprints, live-handle rules. |
| `engine-steward` | Steward config, dossier building, cohort analysis, steward work items. |
| `engine-release` | Release coordination, publish/sign-off receipts, distribution hooks. |
| `engine-chainworks-workflow` | Chainworks proposal/review/implementation workflow extensions and prompt context. |

This does not require all six crates on day one. The implementation can begin as modules inside `engine`, then extract crates once dependency direction is clear.

### 5.7 Workflow extension contracts

Generic workflow execution should call typed extension points instead of embedding state IDs:

```rust
pub trait WorkflowRuntimeExtension {
    fn extra_prompt_context(
        &self,
        run: &RunContext,
        state: &CompiledState,
    ) -> anyhow::Result<Vec<PromptContextBlock>>;

    fn on_state_entered(
        &self,
        event: StateEntered,
    ) -> anyhow::Result<Vec<RuntimeAction>>;

    fn on_stage_blocked(
        &self,
        event: StageBlocked,
    ) -> anyhow::Result<Vec<RuntimeAction>>;
}
```

The Chainworks proposal workflow extension can own:

- rejected implementation approval context;
- code-writer handoff status;
- blocked-before-code behavior;
- implementation review synthetic artifacts;
- proposal closeout/release-specific runtime actions.

The generic engine should only know that an extension returned prompt context or runtime actions.

### 5.8 Capability registry

Create one registry that covers:

- `CapabilityToolId`;
- MCP tool name and params/result type marker;
- MCP resource template ID, when applicable;
- GraphQL mutation name, when still exposed for UI;
- default principal policy;
- command-journal caller label;
- deprecation or UI-only/agent-only boundary notes.

Acceptance rule: adding a mutating tool, resource, or mutation without a registry entry must fail tests.

---

## 6. Implementation Plan

### Phase 0: Inventory and architecture gates

- Add `proposal-070|p070` gate alias.
- Add a dependency-shape check based on `cargo metadata`.
- Fail if `mcp-server` depends on `graphql-server`.
- Document allowed crate dependency layers in `docs/reference/rust-control-plane.md`.
- Add a file-size inventory report to the gate output, warning on central files above the threshold chosen for this proposal.

This phase should not move behavior.

### Phase 1: Extract neutral surface contracts

- Move `RequestId`, request-id validation, and request-id middleware helpers out of `graphql-server`.
- Update GraphQL and MCP HTTP to use the neutral shared type.
- Move capability/tool/resource registry metadata into the shared surface crate.
- Keep existing GraphQL and MCP behavior unchanged.
- Delete the `mcp-server -> graphql-server` dependency.

### Phase 2: Type work-item payloads

- Introduce `WorkItemPayload` and per-kind structs.
- Add compatibility fixtures for existing payload shapes.
- Update enqueue sites to serialize typed payloads.
- Update executor and scheduler parsing to deserialize typed payloads once at the boundary.
- Keep `work_items.payload_json` as the storage column.

### Phase 3: Type MCP tool params and results

- Convert MCP tools one family at a time: `runs`, `stages`, `approvals`, `reports`, `artifacts`, `steward`, `ideas`.
- Each family gets typed params/results and schema parity tests.
- Preserve JSON-RPC envelope and current tool names.
- Add compact/default include behavior where P063 requires it.

### Phase 4: Extract shared read models

- Create typed read DTOs for run/stage/artifact/scheduler/daemon/recovery views.
- Move enrichment logic currently duplicated in GraphQL/MCP into the shared query layer.
- Update GraphQL resolvers and MCP tools/resources to adapt from shared DTOs.
- Add parity tests for fields intentionally shared by both surfaces.

### Phase 5: Split engine by behavior ownership

- First split central files into modules with narrow public APIs.
- Move session, steward, release, artifact settlement, and workflow extension logic behind explicit traits/services.
- Extract crates only after module dependencies are acyclic and clear.
- Keep public re-exports for downstream callers until follow-up cleanup proposals can remove compatibility aliases.

### Phase 6: Move Chainworks workflow policy behind extensions

- Introduce workflow runtime extension contracts.
- Move proposal/review/implementation-loop state-specific policy out of generic orchestrator code.
- Add tests proving the generic engine can execute a workflow without Chainworks-specific state IDs.
- Add tests proving the Chainworks extension preserves current proposal workflow behavior.

### Phase 7: Reorganize tests and docs

- Rehome large proposal-era tests into behavior suites.
- Keep short proposal compatibility tests only where they document a specific acceptance criterion.
- Update reference docs after behavior is implemented:
  - `docs/reference/rust-control-plane.md`;
  - `docs/reference/workflow-execution-engine.md`;
  - `docs/reference/mcp-northbound-control-plane-server.md`;
  - `docs/reference/query-projections-and-client-consumption-contract.md`;
  - `docs/reference/execution-truth-and-recovery.md`;
  - `docs/reference/test-gates.md`.

---

## 7. Acceptance Criteria

- **AC-1. Dependency boundary:** `cargo metadata` proves `mcp-server` does not depend on `graphql-server`; both depend only on neutral shared crates and runtime/query services.
- **AC-2. Work-item typing:** all `WorkItemKind` payloads deserialize into `WorkItemPayload`; executor and scheduler code no longer perform business logic through `serde_json::Value` string-key reads.
- **AC-3. MCP typing:** each MCP tool family has typed params/results and tests that match the advertised input schema.
- **AC-4. Read-model parity:** GraphQL and MCP shared run/stage/artifact fields originate from the same read-model DTOs, with parity fixtures for representative active, blocked, failed, completed, and stale runs.
- **AC-5. Engine ownership:** central engine runtime paths are split so generic workflow transition code does not own ACP invocation, artifact settlement, session policy, release coordination, and Chainworks-specific prompt context in the same function.
- **AC-6. Workflow extensions:** no generic engine code checks for Chainworks proposal state IDs such as `state_5_proposal_refined` or `state_6_implementation_approval`.
- **AC-7. Capability registry:** every MCP tool/resource and GraphQL mutation with durable effect maps through one capability registry, with principal policy coverage.
- **AC-8. Tests remain green:** `./scripts/test-gate.sh fast` and the new `./scripts/test-gate.sh proposal-070` pass.
- **AC-9. Docs match code:** reference docs describe the implemented crate graph, boundary rules, typed internal contracts, and shared read-model ownership.

---

## 8. Validation Strategy

The `proposal-070|p070` gate should include:

- `cargo test --workspace` or the repository's fast Rust subset selected by `scripts/test-gate.sh`;
- dependency-shape check from `cargo metadata`;
- typed work-item payload round-trip fixtures;
- MCP schema/params/result parity tests;
- GraphQL/MCP read-model parity tests;
- capability registry completeness tests;
- workflow extension test that proves generic engine has no Chainworks-specific state IDs;
- docs reference check for P070-owned files.

Long-running benchmarks, load tests, remote UI tests, and daemon soak tests are not required for the initial refactor gate unless a phase changes runtime scheduling semantics.

---

## 9. Rollout and Safety

P070 must be implemented in small behavior-preserving steps:

- Prefer adding typed wrappers and adapter layers before deleting legacy JSON paths.
- Maintain decode compatibility for existing pending/running `work_items.payload_json` rows.
- Keep public server response shapes stable unless a follow-up proposal explicitly changes them.
- Do not clean `.chainworks`, SQLite DB files, run-owned worktrees, artifacts, build output, or target directories as part of this refactor.
- For any lifecycle or cleanup behavior touched during engine extraction, preserve the repository safety rule: dirty run-owned work must have a durable branch, commit, patch bundle, archive, or explicit operator discard decision before destructive lifecycle action.
- If an extraction phase discovers behavior drift, stop at the module boundary and add characterization tests before continuing.

---

## 10. Open Questions

1. Should `control-plane-surface` own only HTTP/request/capability contracts, or should MCP tool metadata also live there?
2. Should `control-plane-read-model` be a new crate, or should it start as a `db::read_model` module and become a crate after duplication is removed?
3. Should P056 be closed as superseded by P070, or kept as a smaller prerequisite for typed work items and engine modularization?
4. What file-size threshold should become a warning in the proposal gate: 2,000 LOC for runtime files, 2,500 LOC, or a per-domain threshold?
5. Should workflow extension contracts be part of `workflow` or `engine-core`?

---

## 11. Expected Outcome

After P070, the control-plane should still expose the same operator behavior, but future proposals should have clear ownership paths:

- surface changes land in MCP/GraphQL adapters plus shared surface contracts;
- query changes land in read-model DTOs once;
- execution changes land in engine runtime services;
- workflow-family policy lands in workflow extensions;
- storage changes land in db repos and migrations;
- authorization changes land in one capability registry and policy test suite.

That is the durable value of this proposal: it turns accumulated proposal-era implementation knowledge into architecture that can absorb the next round of product work without continuing to grow the same central files.
