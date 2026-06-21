# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Chainworks Forge is a macOS SwiftUI **control plane for agent-driven workflows**. The primary object is a **Run**, not a chat thread. A run takes one idea, compiles a frozen workflow snapshot from YAML, routes work through specialized agents (Codex / Claude Code / Gemini via ACP), pauses at explicit approval gates, stores durable artifacts on disk, and leaves behind a structured report.

There are two coexisting codebases in this repository:

1. **SwiftUI app** (canonical owner, production): `Chainworks Forge/` — the operator-facing app built on Xcode + SwiftData. This is what users launch.
2. **Rust + SQLite control-plane daemon** (parity replica): `control-plane/` — a single Rust binary that mirrors orchestration, persistence, and boundary shape. Per P027, this runs alongside the app; the Swift app remains the canonical operator shell during parity. See [`docs/reference/rust-control-plane.md`](docs/reference/rust-control-plane.md).

## Build, test, run

### Swift app (primary)

**Everything funnels through `scripts/test-gate.sh`**, never raw `xcodebuild -testPlan ...`. The gates encode the canonical proving path and pin test selection.

```bash
./scripts/test-gate.sh list       # show all gates
./scripts/test-gate.sh build      # compile-only sanity check
./scripts/test-gate.sh fast       # guardrails + build + high-ROI unit/runtime tests (default inner loop)
./scripts/test-gate.sh guardrails # cheap source-tree lints (no build)
./scripts/test-gate.sh full       # full xcodebuild test sign-off gate
./scripts/test-gate.sh proposal-XXX   # other focused proposal proof gates (see `list`)
```

**UI tests are remote-only by repo policy.** They run over SSH against an approved remote macOS host (`test@SMacBook.local`), not locally. See [`docs/reference/agent-ui-test-execution.md`](docs/reference/agent-ui-test-execution.md) and [`docs/reference/test-gates.md`](docs/reference/test-gates.md) for the full protocol. Typical remote gate invocation:

```bash
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"
```

To open the project in Xcode: `open "Chainworks Forge.xcodeproj"` (target deployment: macOS 26.2+, Xcode 26.3+).

### Rust control-plane daemon

Use the managed Cargo policy for all Rust work. Do **not** run raw `cargo`
with a local `control-plane/target` or worktree-local target unless you are
doing one-off cache diagnosis and explicitly set
`CHAINWORKS_ALLOW_LOCAL_CARGO_TARGET_DIR=1`.

```bash
cd control-plane
../scripts/cargo-managed build
../scripts/cargo-managed test              # full workspace tests
../scripts/cargo-managed test -p engine <test_name>  # single test
cd .. && ./scripts/test-gate.sh proposal-027   # canonical P027 regression gate
```

For longer interactive sessions, external agents may instead source the policy
once before running Cargo:

```bash
source scripts/cargo-cache-env.sh
cd control-plane
cargo test -p engine <test_name>
```

The policy sets a shared bounded `CARGO_TARGET_DIR`, enables `sccache` when
available, and installs a `cargo` wrapper that remaps accidental
`CARGO_TARGET_DIR=target/...` gate commands into the shared gate cache.

Run the daemon (GraphQL on `:4000/graphql`, MCP Streamable HTTP on `:4000/mcp`, logs to stderr):

```bash
cd control-plane
DATABASE_URL="sqlite:///Users/user/Documents/Chainworks Forge/.chainworks/control-plane.db?mode=rwc" 
GRAPHQL_ADDR="127.0.0.1:4000" 
RUST_LOG=info,acp=debug 
"${CARGO_TARGET_DIR:-$HOME/Library/Caches/Chainworks Forge/cargo-target}/debug/control-plane" 2>/tmp/cw.log &
```

Connect Claude Code to the running daemon as an MCP server via `.mcp.json` at repo root (`"type": "http"`, `"url": "http://127.0.0.1:4000/mcp"`).

The daemon requires a bearer token for MCP auth (P029). On first start, it auto-creates `~/.chainworks/auth/principals.json` with a default operator token and logs only the path with a redacted note; read the raw token from the principals file. Set `CHAINWORKS_MCP_TOKEN=<token>` in your shell environment before connecting. The `.mcp.json` already includes the `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}` header.

## Architecture: big picture

### Run lifecycle

An `Idea` + YAML `Workflow` + YAML `AgentCatalog` compiles into a **`RunPlanSnapshot`** (frozen at run start — workflow, catalog, provider bindings, path templates all captured together for drift detection on resume). The Swift app is the operator shell and state/artifact reader. Live provider execution is owned by the Rust control plane, which dispatches ACP subprocess sessions through provider-specific adapters for Claude / Codex / Gemini / Auggie / Junie.

Key engine pieces (under `Chainworks Forge/Engine/`):

- `RunPlanCompiler.swift` — compiles idea + YAML → frozen `RunPlan` with resolved agent bindings
- `WorkflowOrchestrator.swift` — state machine driver, lazy stage creation, fan-out via `TaskGroup`, transition authority resolver
- `TransitionEvaluator.swift` — canonical `when:` expression evaluator (ARCH-031): `exists()`, comparisons, `vars.*`, `artifact.field`, `and/or`
- `RuntimeAgentExecutor.swift` — single-agent execution with retries, watchdog, owner-aware execution identity
- `AgentSessionManager.swift` — per-run session lineage, reuse scopes, generation tracking
- `ExecutionService.swift` — top-level coordinator, app-local fixture transport factory
- `RecoveryCoordinator.swift` / `ResumeManager.swift` — startup repair, drift handling

ACP transport lives in the Rust control plane:
- `control-plane/crates/acp/src/transport.rs` — ndjson JSON-RPC subprocess transport
- `control-plane/crates/acp/src/adapters/*.rs` — per-provider launch/session config
- Codex prepares an isolated runtime home (`CODEX_HOME` + copied `auth.json` + sanitized `config.toml`) in `control-plane/crates/acp/src/adapters/codex.rs`

### Rust control-plane (parity replica)

9-crate workspace at `control-plane/crates/`:

```
daemon ─┬─► graphql-server ─┐
        ├─► mcp-server      ├─► engine ─┬─► db ─► SQLite (WAL, projections, work queue)
        │                   │           ├─► auth ─► principal table + boundary caller class
        │                   │           └─► acp ─► ACP subprocess adapters
        │                   └─► workflow ─► YAML compiler (runs & agent catalogs)
        └─► (single binary, both northbound servers on :4000)
```

- `domain` — IDs, enums, command/event types (no I/O), mediation types
- `db` — SQLite repos + projection rebuild logic, versioned migrations, WAL mode for concurrent reads, work queue, and projection metadata
- `auth` — bearer principals, principal-table loading, caller-class derivation, and shared boundary authorization helpers
- `workflow` — parses `examples/workflows/*.yaml` + `examples/agents/*.yaml`, builds `RunPlan` with resolved `ResolvedAgent { provider, model, effort, prompt }`
- `engine` — `Orchestrator` (state machine), `BackgroundExecutor` (work-queue worker), `CommandHandler` (writes to `command_journal`), `MediationSettlementService`, `RecoveryService`
- `acp` — JSON-RPC 2.0 ndjson transport with permission auto-grant, 5 provider adapters
- `graphql-server` — async-graphql + axum; queries, mutations, subscriptions; workflow conflict readback
- `mcp-server` — MCP Streamable HTTP transport (POST /mcp) + stdio fallback; mixed inbox (`approvals.list`) for stage approvals and mediation confirmations

Key reference: [`docs/reference/rust-control-plane.md`](docs/reference/rust-control-plane.md).

### Shared contracts

Both codebases consume the same YAML schemas:
- **Workflow YAML**: `states` map with `owner`, `run` (sequence/parallel/then tasks), `transitions` (condition expressions), optional `loop` with counter+max
- **Agent catalog YAML**: `backend_profiles` (provider/model/effort/mcp) + `agents` list (id, backend_profile, prompt, inputs, outputs, permission_profile)
- **Artifact path map**: `artifacts:` section maps logical names (`proposal_current`) to filesystem templates (`${CHAINWORKS_META_ROOT:-.chainworks}/proposals/current/proposal.md`) — transitions resolve paths against this map

Canonical examples live at `examples/workflows/full-mvp-live.yaml` and `examples/agents/agents.yaml`.

## Documentation map

The doc tree is strict about "aspirational vs implemented":

- **`docs/reference/`** — canonical, implemented-system truth. Start here when you need to understand how something works today.
- **`docs/proposals/NNN-*.md`** — design intent. Each has an implementation audit trail next to it (`NNN-*_IMPLEMENTATION_AUDIT_RN.md`). When a proposal reaches `Implemented/Ready` verdict, it's retired: content is converted into a reference doc and the proposal file is deleted (git retains history).
- **`docs/evidence/`** — proof artifacts for acceptance gates
- **`docs/reviews/`** — review transcripts and evidence packs
- **`docs/README.md`** — full index with reading order

Key entry points:
- [`docs/README.md`](docs/README.md) — documentation index
- [`docs/reference/current-system-baseline.md`](docs/reference/current-system-baseline.md) — subsystem map at current HEAD
- [`docs/reference/workflow-execution-engine.md`](docs/reference/workflow-execution-engine.md) — RunPlan compiler, orchestrator, executors
- [`docs/reference/acp-runtime-transport.md`](docs/reference/acp-runtime-transport.md) — ACP adapter contract
- [`docs/reference/test-gates.md`](docs/reference/test-gates.md) — gate-by-gate semantics

## Conventions

- **Frozen snapshots**: run state compiled at start is frozen in `RunPlanSnapshot`. Drift detected on resume against current YAML → run blocks until engineer chooses a `DriftDecision`.
- **Single active run per idea**: `RunRepository` is the sole approved entry point for creating `Run` records. Direct `Run(...)` construction elsewhere is a contract violation enforced by automated scan tests. See ARCH-002 in [`docs/reference/architecture-decisions.md`](docs/reference/architecture-decisions.md).
- **Artifacts on disk, metadata in SwiftData/SQLite**: the canonical truth for artifact *contents* is the filesystem; DB records are the metadata/index. Transition `exists('name')` checks resolve the artifact path template and check filesystem presence.
- **ACP permission auto-grant**: the Rust transport auto-grants the first `allow_once` option for `session/request_permission` notifications. The `method` check comes **before** the terminal-response `id` match — a fix that matters because `session/request_permission` arrives with `id=0`.
- **Agent prompts include resolved output paths**: agents receive a directive listing required outputs with their canonical filesystem paths so they write directly to the right location, eliminating post-hoc normalization.
- **Goose is legacy**: the long-term transport is ACP (Codex/Claude Code/Gemini). Goose compatibility exists for migration, not as the canonical transport model — see proposals 026 and 030.

## Agent Output Contract Handling

**P079: Contract-Aware Output Repair and Provider Fallback** is partially implemented. The SQLite migration, domain types, DB repos for repair events and leases, GraphQL/MCP readback surfaces, Swift evidence DTOs, MCP runtime receipt sanitization, crash-consistent materialization, Junie plan-evidence capture/redaction, and deterministic fixture same-session repair path are wired. Production same-session repair remains fail-closed for advisory-only providers until the runtime provides enforceable sandbox/permission restrictions. Transcript/provider-envelope recovery, controlled provider fallback, macOS inspector UI, P079 metrics, and the full acceptance gate remain deferred.

For more details, refer to [`docs/reference/output-contracts-failure-evidence-and-recovery.md`](docs/reference/output-contracts-failure-evidence-and-recovery.md).
