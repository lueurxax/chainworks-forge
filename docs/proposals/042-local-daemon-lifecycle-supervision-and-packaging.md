# Proposal 042: Local Daemon Lifecycle, Supervision, and Packaging

| Field | Value |
|---|---|
| Date | 2026-04-11 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 027 |
| Goal | Define how the local Rust daemon is started, supervised, upgraded, observed, and packaged as a real product component. |

## 1. Why this proposal exists

Proposal 027 introduces a local server process.
That is not just an implementation detail.

Once a daemon becomes part of the product, the repo needs explicit answers for:

- startup,
- health,
- restarts,
- logs,
- schema upgrades,
- dev vs packaged behavior,
- and how the desktop client finds and trusts the daemon.

Without this, the control plane may exist in code but still not be operable as a product component.

## 2. Outcome

After Proposal 042:

- the daemon has a defined lifecycle,
- the client has a clear connection and health model,
- restart and supervision behavior is product-owned,
- logs and diagnostics are discoverable,
- schema upgrades are explicit,
- packaged mode and dev mode are both supported intentionally.

## 3. Scope

This proposal includes:

- daemon startup and shutdown rules
- supervision and restart policy
- health endpoints and readiness checks
- logging and diagnostics locations
- SQLite schema upgrade rules
- dev mode vs packaged mode behavior
- packaging and distribution expectations

This proposal does **not** include:

- distributed deployment,
- remote clustering,
- multi-machine control planes,
- or MCP API design.

## 4. Product questions this proposal must answer

1. How does the daemon start in dev and packaged builds?
2. How does the client detect healthy vs unhealthy daemon state?
3. What happens after daemon restart or crash?
4. Where do operators find logs and diagnostic output?
5. How are SQLite schema upgrades applied safely?

## 5. Lifecycle model

The daemon should have explicit lifecycle phases:

- not started
- starting
- ready
- degraded
- restarting
- failed
- shutdown

The client must not invent its own interpretation of daemon liveness outside this contract.

## 6. Supervision model

The system should define:

- who starts the daemon,
- who restarts it,
- when automatic restart is allowed,
- when restart must surface operator-visible failure,
- how runaway restart loops are prevented.

## 7. Packaging model

The repo should distinguish:

- local developer mode,
- app-bundled packaged mode,
- test/fixture mode.

Paths, logs, and startup rules must be explicit for each mode.

## 8. Risks

### 8.1 Dev-only assumptions leak into packaged mode

Risk:
- the daemon works only from a source checkout.

Mitigation:
- define packaged paths, bundled assets, and startup discovery explicitly.

### 8.2 Health checks become transport-specific hacks

Risk:
- the client reintroduces local heuristics and hidden fallbacks.

Mitigation:
- one daemon lifecycle contract,
- one health/readiness model,
- explicit degraded/failure states.

## 9. Acceptance criteria

Proposal 042 is complete when:

1. daemon lifecycle states are defined and implemented,
2. client connection and health behavior is explicit,
3. restart and crash recovery behavior is documented and testable,
4. logs and diagnostics have stable locations,
5. SQLite schema upgrade behavior is defined,
6. dev mode and packaged mode are both intentionally supported.

## 10. Final recommendation

Proposal 042 should land before thin-client cutover.

Without it, the server may exist but still fail as a dependable local product component.
