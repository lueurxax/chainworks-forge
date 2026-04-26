# Proposal 055: Control-Plane Debug Launch Supervision Hardening

| Field | Value |
|---|---|
| Date | 2026-04-18 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [local daemon lifecycle contract](../reference/local-daemon-lifecycle-supervision-and-packaging.md), [037-acp-execution-supervision-and-idle-watchdog.md](037-acp-execution-supervision-and-idle-watchdog.md) |
| Scope | Harden the developer/debug launch path for the Rust control-plane daemon so local restarts are deterministic, observable, and bounded when the process stalls before Rust `main`. |
| Goal | Make debug supervision a single-owner, absolute-path, readiness-checked path that cannot silently respawn a stuck pre-main process or truncate the only useful logs. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-055|p055`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Evidence

During live recovery for run `9318de0d-9c75-40ad-9d0a-74c3610b021d` on 2026-04-18, the local debug daemon restart path failed in a way that the packaged daemon lifecycle contract intentionally does not own: a developer/debug LaunchAgent can stall before Rust `main`, before in-process lifecycle readback exists.

Observed sequence:

- The existing debug LaunchAgent label was `com.chainworks.forge.control-plane.codex-debug`.
- The submitted command used a shell `cd` into `control-plane`, then launched `./target/debug/control-plane` with relative paths and `> /tmp/cw.log 2>&1`.
- `launchd` `KeepAlive` repeatedly respawned child processes that appeared stuck before Rust `main`.
- Samples showed the process in dyld/open/getcwd style startup frames before any Rust lifecycle log line.
- Each respawn could truncate `/tmp/cw.log`, destroying startup evidence.
- `launchctl bootout gui/$(id -u)/com.chainworks.forge.control-plane.codex-debug` stopped the respawn loop.
- Starting the already-built binary from the repository root with an absolute binary path, absolute env paths, append-only logs, and Python `subprocess.Popen(..., start_new_session=True)` worked.
- The working daemon reported `/ready` 200 with PID `23129`.

This is not enough evidence to claim a universal macOS dyld or launchd defect. It is enough to show that the current debug launch shape is brittle and that the product needs a bounded, inspectable startup contract for developer and dogfood runs.

## 2. Problem Statement

### 2.1 Debug launch relies on relative process state

The failing command depended on shell `cd` and a relative binary path:

```text
cd ".../control-plane" && exec env ... ./target/debug/control-plane
```

If launchd, dyld, sandbox state, deleted directories, or inherited cwd state behaves unexpectedly, the daemon can stall before the Rust lifecycle reporter exists. The in-process `/health` and `/ready` contracts cannot report a failure that happens before Rust `main`.

### 2.2 KeepAlive can hide a pre-main startup stall

The debug supervisor kept respawning processes without a readiness deadline. From the operator's perspective this looked like "the daemon is restarting" rather than "the launch path is stuck before daemon startup". This is especially dangerous during run recovery, because stale running work items can be repaired or retried while the operator is working from incomplete logs.

### 2.3 Log truncation destroys root-cause evidence

Using `>` for `/tmp/cw.log` makes every respawn overwrite the prior startup trace. For pre-main failures, the log might be the only artifact that distinguishes:

- daemon code failing after lifecycle reporter startup,
- dynamic loader or process launch stalling before Rust code,
- duplicate launch owner conflict,
- wrong database/catalog/steward config env,
- shell/cwd/path quoting errors.

## 3. Scope

P055 covers:

- debug and dogfood daemon launch commands used by local operators, Codex sessions, and control-plane development scripts
- absolute binary and environment path resolution
- single-owner start/stop semantics for the debug LaunchAgent label
- readiness deadline enforcement for child startup
- bounded handling for pre-Rust-main startup stalls
- append-only or rotated logs for debug restarts
- diagnostic readback that names launch mode and supervisor owner
- a canonical `proposal-055|p055` proof gate

P055 does not cover:

- production packaging, signing, notarization, or SMAppService release policy beyond respecting the local daemon lifecycle and packaging contract
- changing run retry, stage retry, or recovery semantics
- changing ACP transport, Xcode MCP bridge pooling, or provider session reuse
- solving macOS dyld internals
- deleting historical `.chainworks` state or run artifacts

## 4. Core Rules

### 4.1 Absolute launch contract

Every debug supervised launch must materialize an absolute launch descriptor before spawning:

- absolute daemon binary path
- absolute repository root
- absolute `DATABASE_URL` SQLite path
- absolute `AGENT_CATALOG_PATH`, when provided
- absolute `STEWARD_CONFIG_PATH`, when provided
- absolute log paths
- explicit `GRAPHQL_ADDR`
- explicit `RUST_LOG`
- explicit launch mode, for example `CHAINWORKS_LAUNCH_MODE=debug_launchd`

The implementation must not depend on shell `cd` to make relative paths work.

### 4.2 Single supervisor owner

The debug start path must first discover whether a known debug LaunchAgent label already exists. If it owns that label, it must update or boot out the old job before starting a new one. If another owner is detected, it must fail visibly and print the competing owner details.

Required invariant:

```text
start_debug_daemon() never leaves two live control-plane daemon processes for the same database.
```

### 4.3 Readiness deadline outside the daemon

The supervisor must wait for `GET /ready` after launch. If readiness does not return 200 within the configured deadline, the launch attempt is failed from the supervisor side.

The failure report must include:

- launch descriptor hash
- PID, if a child exists
- elapsed startup time
- last known process state
- last log tail path
- whether `/health` answered
- whether `/ready` answered
- whether the process reached Rust lifecycle logging

This is intentionally outside the daemon because pre-main failures cannot be represented by in-process `DaemonLifecycleState`.

### 4.4 Pre-main stall is bounded

If a child process exists but neither lifecycle logging nor `/health` appears before the deadline, the supervisor treats it as a `pre_main_startup_stall`.

For debug mode the supervisor may terminate that child after collecting diagnostics. It must not rely on launchd `KeepAlive` to keep trying forever.

### 4.5 Logs are append-only or rotated

Debug launches must not truncate the primary log on each retry. Acceptable policies:

- append to a stable log and record per-attempt separators with timestamps
- write one file per launch attempt under a bounded retention directory
- rotate by size and count before starting a new attempt

The gate must prove that a failed restart attempt preserves prior log content.

## 5. Required Behavior

### 5.1 Canonical debug launcher

Add one canonical debug launcher entrypoint. It may be a script or a Rust helper, but all local guidance and agent instructions must use it rather than hand-written shell snippets.

Suggested command shape:

```bash
./scripts/control-plane-debug-daemon.sh start
./scripts/control-plane-debug-daemon.sh status
./scripts/control-plane-debug-daemon.sh stop
```

The exact filename can change during implementation, but the proposal requires a single documented owner.

### 5.2 Launch descriptor serialization

Before spawning, the launcher writes a redacted descriptor artifact under a debug diagnostics directory, for example:

```text
.chainworks/diagnostics/control-plane-debug-launch/current.json
```

The descriptor includes all path and mode fields from §4.1. It redacts bearer tokens and secrets.

### 5.3 Health/readiness integration

The launcher must use:

- `/health` to decide whether a process has reached daemon lifecycle serving
- `/ready` to decide whether the daemon can accept work

A process that reaches `/health` 200 but `/ready` 503 is not a pre-main stall. It is a daemon lifecycle failure or degraded state and belongs to the typed daemon status path.

### 5.4 Debug status readback

At least one debug surface must report launch metadata:

- launch mode
- supervisor owner
- launcher version or descriptor hash
- binary path
- database path
- log path
- PID
- last readiness result

This can be exposed through `/ready`/`/health` extensions, GraphQL `daemonStatus`, or a local status command. Implementation must pick one and test it.

## 6. Implementation Inventory

Expected files and areas:

| Area | Expected ownership |
|---|---|
| `control-plane/crates/daemon/src/supervisor.rs` | Launch descriptor types, readiness deadline helpers, pre-main stall classification, process-owner checks if implemented in Rust |
| `control-plane/crates/daemon/src/main.rs` | Launch mode readback and lifecycle metadata wiring |
| `control-plane/crates/graphql-server/src/server.rs` | Optional debug status readback if implemented through health/ready or GraphQL |
| `scripts/test-gate.sh` | Register `proposal-055|p055` |
| `docs/reference/test-gates.md` | Document `proposal-055|p055` |
| `docs/reference/rust-control-plane.md` | Update developer daemon run guidance to use the canonical debug launcher |
| `docs/reference/local-daemon-lifecycle-supervision-and-packaging.md` | Update supervision truth without duplicating the stable lifecycle contract |
| `scripts/control-plane-debug-daemon.sh` or equivalent | Canonical developer launcher, if the team chooses a script entrypoint |
| `control-plane/crates/daemon/tests/` | Focused descriptor/readiness/stall/log-preservation tests |

If implementation discovers that the local daemon lifecycle reference already owns the best destination for some of these changes, P055 should extend that reference rather than create duplicate lifecycle truth.

## 7. Proof Gate

`./scripts/test-gate.sh proposal-055` must run locally and must not require Xcode, a simulator, or a packaged release host.

The gate must include focused proof for:

1. Launch descriptor generation uses absolute paths and no shell `cd` dependency.
2. Existing debug LaunchAgent ownership is detected before start.
3. Starting twice does not leave duplicate daemon processes for the same database.
4. A child that never reaches lifecycle logging or `/health` before the deadline is classified as `pre_main_startup_stall`.
5. A child that reaches `/health` but not `/ready` is not misclassified as pre-main.
6. Failed restart attempts preserve prior log content.
7. Debug status readback includes launch mode, supervisor owner, PID, database path, and log path.
8. `docs/reference/test-gates.md` and `scripts/test-gate.sh` register `proposal-055|p055`.

The gate should finish with `cargo test --workspace` inside `control-plane/` unless implementation scope proves a narrower regression is sufficient and the proposal is amended.

## 8. Acceptance Criteria

- Local debug daemon startup succeeds from any current working directory.
- The launcher uses absolute paths for binary, database, catalog, steward config, and logs.
- The launcher does not submit a stale debug LaunchAgent alongside a new owner.
- A startup stall before Rust `main` fails within a bounded timeout and leaves diagnostics.
- Restart attempts append or rotate logs without truncating the previous attempt's evidence.
- `/ready` reaches 200 before the launcher reports success.
- `ps` or an equivalent tested process inventory shows at most one daemon for the selected database.
- Operator guidance no longer recommends ad hoc `cargo build & ... ./target/debug/control-plane 2> /tmp/cw.log` snippets as the canonical local debug path.

## 9. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| The observed launchd/dyld stall is environment-specific and hard to reproduce | Test descriptor generation, readiness timeout, and fake-child stall classification directly; keep live launchd proof as an integration smoke, not the only evidence |
| A script launcher and Rust supervisor drift apart | Pick one canonical launcher; if both exist, make one call into the other or share a generated descriptor format |
| Over-eager stall cleanup kills a slow but valid startup | Use separate "no lifecycle evidence" and "health but not ready" classifications; make timeout configurable for debug runs |
| Logs grow without bound | Use bounded rotation or retention; append-only does not mean infinite retention |
| P055 duplicates daemon lifecycle truth | Keep P055 focused on debug launch supervision before in-process daemon lifecycle is available; delegate in-process lifecycle semantics back to the stable local daemon reference |

## 10. Open Questions

- Should the canonical launcher be a shell script for operator ergonomics, a Rust subcommand for stronger typing, or both with the script delegating to Rust?
- Should debug launch status be exposed through `/health`/`/ready` JSON, GraphQL `daemonStatus`, or only the local status command?
- What is the default readiness deadline for debug runs: 10 seconds, 30 seconds, or inherited from existing daemon lifecycle config?
- Should the launcher manage launchd by default, or should launchd be opt-in while the default debug path uses direct process supervision?
