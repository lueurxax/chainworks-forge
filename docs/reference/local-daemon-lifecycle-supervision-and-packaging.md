# Local Daemon Lifecycle, Supervision, and Packaging

This document is the implemented contract for the local Rust control-plane daemon as a macOS product component. It replaces the retired proposal-era daemon lifecycle document as the stable source of truth.

| Field | Value |
|---|---|
| Implementation status | Implemented |
| Implementation readiness | Ready |
| Release packaging readiness | Pending release-host packaging evidence |
| Implementation gate | `./scripts/test-gate.sh proposal-042` |
| Release-host gate | `./scripts/test-gate.sh proposal-042-packaging` |
| Implementation evidence | `docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log` |
| Release evidence directory | `docs/evidence/042-local-daemon-lifecycle/` |

The gate names retain the historical `proposal-042` and `p042` aliases because the test runner and proof logs use those stable identifiers. They are operational gate names, not active proposal dependencies.

## Scope

The local daemon contract covers:

- typed lifecycle state and failure readback;
- `/health`, `/ready`, GraphQL `daemonStatus`, and GraphQL `daemonStatusChanged`;
- macOS packaged-app and packaged-helper supervision;
- singleton PID locking, crash-loop budget handling, and graceful shutdown;
- packaged-mode path resolution, loopback binding, `daemon.port`, and `build-sha.txt`;
- SQLite migration preflight, backups, no-downgrade safety, and failed-serve mode;
- log routing, redaction, retention, diagnostics export, and request-id correlation;
- implementation and release-host proof lanes.

It does not cover:

- multi-host daemon deployment;
- remote orchestration or clustering;
- thin-client UI cutover ownership;
- GraphQL projection read-model ownership beyond daemon lifecycle status;
- production release side effects for workflow release stages.

## Runtime modes

| Mode | Owner | Paths | Restart behavior |
|---|---|---|---|
| `dev` | Developer process | Repo-relative defaults, stderr logs | Manual restart |
| `packaged-app` | Chainworks Forge app using `SMAppService.Agent` | `$HOME/Library/Application Support/Chainworks Forge/` and `$HOME/Library/Logs/Chainworks Forge/` | App-owned supervision |
| `packaged-helper` | Per-user LaunchAgent | Same app-support and log roots as packaged app | launchd `KeepAlive` plus daemon-side crash budget |
| `test` | Test harness | In-memory or temporary roots | Harness-owned |
| `mcp` | Legacy MCP stdio compatibility path | Dev-style paths | Caller-owned |

Packaged modes chdir to `$HOME` before serving so relative defaults cannot reach back into a source checkout or launcher's working directory. Dev, test, and MCP modes keep the caller's cwd.

## Lifecycle state

The shared lifecycle shape is owned by `domain::lifecycle` and consumed by the daemon, GraphQL server, MCP server, Swift app, and tests.

| State | Meaning | Liveness |
|---|---|---|
| `not_started` | Process object exists but lifecycle has not entered startup | Not live |
| `starting` | Startup is in progress | Not ready |
| `ready` | Daemon can serve normal work | Live and ready |
| `degraded` | Daemon is alive but a subsystem is impaired | Live, not ready |
| `restarting` | Restart transition is being observed | Not ready |
| `failed` | Current process hit a terminal daemon-owned failure | Terminal |
| `shutdown` | Graceful shutdown is in progress or complete | Terminal |

`DegradedKind` and `FailureKind` are intentionally separate types. Recoverable degraded conditions cannot accidentally become terminal failure reasons, and terminal failures cannot be reported as degraded warnings.

`DaemonStatus.failure` is populated only when `state == failed`. `DaemonStatus.degraded` is non-empty only when `state == degraded`.

## Readback surfaces

| Surface | Auth | Ready | Degraded | Failed |
|---|---|---|---|---|
| `GET /health` | Unauthenticated loopback probe | HTTP 200 with status JSON | HTTP 200 with degraded JSON | HTTP 503 with failure JSON |
| `GET /ready` | Unauthenticated loopback probe | HTTP 200 with status JSON | HTTP 503 with degraded JSON | HTTP 503 with failure JSON |
| GraphQL `daemonStatus` | Operator bearer auth | Typed status | Typed status | Typed status with `failure` |
| GraphQL `daemonStatusChanged` | Operator bearer auth | Pushes every transition | Pushes every transition | Pushes every transition |

Clients that need live lifecycle state use the snapshot-plus-subscribe pattern:

1. Call `daemonStatus` to get the current snapshot.
2. Subscribe to `daemonStatusChanged`.
3. On subscription lag, disconnect, call `daemonStatus` again, then resubscribe.

Subscription frames and snapshot responses carry the same logical `DaemonStatus` shape. A client must not infer daemon lifecycle state from arbitrary request failures.

## Supervision

### PID lock

Packaged modes acquire `daemon.pid` under Application Support before binding HTTP.

| Outcome | Behavior |
|---|---|
| Lock acquired | Rewrite the file with current PID and continue startup. |
| Lock held by live peer | Exit 0. This is a duplicate-healthy singleton outcome, not daemon failure. |
| Lock held but recorded PID is dead | Exit 75. This is supervisor-owned because the daemon cannot bind readback surfaces before the lock succeeds. |

The lock file is both the advisory mutex and the owner-PID record. The implementation uses open-or-create plus non-blocking exclusive flock; it does not use `O_EXCL`, because duplicate and stale-lock branches must be inspectable.

### Crash-loop budget

Packaged modes track crashes in `crash-budget.json`.

Five crashes within 60 seconds put the daemon into failed-serve mode with `FailureKind::CrashLoopBudgetExhausted`. The process stays alive and serves status surfaces instead of exiting into an infinite supervisor respawn loop.

A clean `ready` period of 5 minutes clears the budget. The Swift app also exposes an operator reset flow that terminates the daemon, removes `crash-budget.json`, and re-registers the packaged agent.

### Graceful shutdown

SIGTERM and SIGINT trigger a two-phase shutdown:

1. Emit `shutdown`.
2. Drain in-flight background work with a 5-second deadline.
3. Gracefully stop HTTP servers with a 5-second deadline.
4. Close the SQLite pool.
5. Release the PID lock by dropping the guard.

Exit code 0 means clean drain. Exit code 75 means drain timeout.

## Packaged paths and binding

Packaged daemon binaries live next to the Swift app executable:

```text
Chainworks Forge.app/Contents/MacOS/Chainworks Forge
Chainworks Forge.app/Contents/MacOS/chainworks-forge-daemon
```

The Swift app discovers the daemon relative to `Bundle.main`; no absolute source-checkout path is used.

Packaged daemons bind loopback only. If port 4000 is occupied by a non-packaged listener, the daemon falls back to an ephemeral loopback port and writes the selected port to:

```text
$HOME/Library/Application Support/Chainworks Forge/daemon.port
```

The Swift client reads `daemon.port` before connecting and falls back to port 4000 only when the file is absent.

## SQLite startup safety

Startup runs migration preflight before normal pool open.

| DB state | Behavior |
|---|---|
| Missing or zero-byte DB | Clean install; apply migrations from scratch. |
| Existing DB without `_sqlx_migrations` | Fail closed with `MigrationFailed`; do not mutate. |
| Applied versions equal binary versions | No-op and continue. |
| Applied versions are a strict subset | Write a backup, then apply migrations under exclusive lock. |
| Applied versions are newer than binary | Fail closed with `SchemaNewerThanBinary`; do not downgrade. |
| Applied versions diverge from binary | Fail closed with `MigrationFailed`; do not mutate. |

Backups are written only for the tracked-subset branch, because that is the only branch that mutates an existing populated database.

Every migration startup failure enters failed-serve mode instead of panicking or exiting. `/health`, `/ready`, and `daemonStatus` remain available so the operator can read the typed failure and any backup path.

## Failed-serve mode

Failed-serve mode is the daemon-owned terminal readback state for failures that occur after the daemon can still bind a status listener.

It serves:

- `/health`;
- `/ready`;
- GraphQL `daemonStatus` only;
- MCP JSON-RPC error envelopes for work requests.

Non-status GraphQL requests and work-producing MCP calls are refused with typed protocol envelopes. Failed-serve mode still uses bearer auth for protected status surfaces.

## Logs, diagnostics, and request correlation

Packaged mode writes JSON logs under:

```text
$HOME/Library/Logs/Chainworks Forge/daemon.log
```

The log writer applies write-time redaction for bearer tokens, principal tokens, home-rooted absolute paths, and packaged database paths. Retention is bounded by age, count, and total bytes.

Inbound GraphQL, MCP HTTP, `/health`, and `/ready` requests receive a safe `X-Request-ID`. The same request id is available to handlers, log spans, MCP error payloads, and command-journal rows.

The Swift diagnostics bundle can be exported even when the daemon is failed or unavailable. It reads local files, redacts principal tokens, includes build/status/log evidence when present, and avoids network calls during export.

## Proof lanes

### Implementation gate

Use:

```bash
./scripts/test-gate.sh proposal-042
```

This gate runs:

- 126 focused Rust tests across `domain`, `engine`, `graphql-server`, `daemon`, `db`, and `mcp-server`;
- the Swift focused lane for daemon lifecycle client, diagnostics bundle, packaged binary checks, supervisor behavior, and crash-budget reset;
- full Rust workspace regression.

The implementation-ready evidence log is:

```text
docs/evidence/042-local-daemon-lifecycle/proposal-042-gate-20260420T063230Z.log
```

### Release-host packaging gate

Use on a configured release host only:

```bash
./scripts/test-gate.sh proposal-042-packaging
```

This lane requires `scripts/packaging.env` with release-host credentials and expected Team ID. It verifies:

- Release archive/export;
- embedded daemon binary presence and executable bit;
- `codesign --verify --deep --strict`;
- matching Developer ID Application authority between app and embedded daemon;
- expected Team ID;
- notarization staple;
- Gatekeeper assessment;
- launch-to-Ready from the packaged app.

Each successful release-host run writes a timestamped evidence log under `docs/evidence/042-local-daemon-lifecycle/`.
